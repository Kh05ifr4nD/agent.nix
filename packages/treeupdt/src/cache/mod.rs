use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry<T> {
    data: T,
    timestamp: SystemTime,
}

impl<T> CacheEntry<T> {
    fn new(data: T) -> Self {
        Self {
            data,
            timestamp: SystemTime::now(),
        }
    }
    
    fn is_expired(&self, ttl: Duration) -> bool {
        self.timestamp.elapsed().unwrap_or(Duration::MAX) > ttl
    }
}

pub struct Cache {
    cache_dir: PathBuf,
    ttl: Duration,
}

impl Cache {
    pub fn new() -> Result<Self> {
        let cache_dir = dirs::cache_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine cache directory"))?
            .join("treeupdt");
            
        std::fs::create_dir_all(&cache_dir)?;
        
        Ok(Self {
            cache_dir,
            ttl: Duration::from_secs(3600), // 1 hour default TTL
        })
    }
    
    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.ttl = ttl;
        self
    }
    
    fn cache_key(&self, source_type: &str, identifier: &str, operation: &str) -> String {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(source_type.as_bytes());
        hasher.update(b":");
        hasher.update(identifier.as_bytes());
        hasher.update(b":");
        hasher.update(operation.as_bytes());
        format!("{:x}", hasher.finalize())
    }
    
    fn cache_path(&self, key: &str) -> PathBuf {
        self.cache_dir.join(format!("{}.json", key))
    }
    
    pub fn get<T: for<'de> Deserialize<'de>>(&self, source_type: &str, identifier: &str, operation: &str) -> Option<T> {
        let key = self.cache_key(source_type, identifier, operation);
        let path = self.cache_path(&key);
        
        if !path.exists() {
            return None;
        }
        
        let content = std::fs::read_to_string(&path).ok()?;
        let entry: CacheEntry<T> = serde_json::from_str(&content).ok()?;
        
        if entry.is_expired(self.ttl) {
            // Clean up expired entry
            let _ = std::fs::remove_file(&path);
            return None;
        }
        
        Some(entry.data)
    }
    
    pub fn set<T: Serialize>(&self, source_type: &str, identifier: &str, operation: &str, data: &T) -> Result<()> {
        let key = self.cache_key(source_type, identifier, operation);
        let path = self.cache_path(&key);
        
        let entry = CacheEntry::new(data);
        let content = serde_json::to_string_pretty(&entry)?;
        std::fs::write(path, content)?;
        
        Ok(())
    }
    
    pub fn clear(&self) -> Result<()> {
        if self.cache_dir.exists() {
            for entry in std::fs::read_dir(&self.cache_dir)? {
                let entry = entry?;
                if entry.path().extension().and_then(|s| s.to_str()) == Some("json") {
                    std::fs::remove_file(entry.path())?;
                }
            }
        }
        Ok(())
    }
}

// Wrapper for caching source results
use crate::sources::{Source, Version, UpdateInfo};
use async_trait::async_trait;

pub struct CachedSource<S: Source> {
    inner: S,
    cache: Cache,
    source_name: String,
}

impl<S: Source> CachedSource<S> {
    pub fn new(inner: S, source_name: String) -> Result<Self> {
        Ok(Self {
            inner,
            cache: Cache::new()?,
            source_name,
        })
    }
    
    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.cache = self.cache.with_ttl(ttl);
        self
    }
}

#[async_trait]
impl<S: Source> Source for CachedSource<S> {
    async fn get_latest_version(&self, identifier: &str) -> Result<Version> {
        // Check cache first
        if let Some(version) = self.cache.get::<Version>(&self.source_name, identifier, "latest_version") {
            return Ok(version);
        }
        
        // Fetch from source
        let version = self.inner.get_latest_version(identifier).await?;
        
        // Cache the result
        let _ = self.cache.set(&self.source_name, identifier, "latest_version", &version);
        
        Ok(version)
    }
    
    async fn get_versions(&self, identifier: &str) -> Result<Vec<Version>> {
        // Check cache first
        if let Some(versions) = self.cache.get::<Vec<Version>>(&self.source_name, identifier, "versions") {
            return Ok(versions);
        }
        
        // Fetch from source
        let versions = self.inner.get_versions(identifier).await?;
        
        // Cache the result
        let _ = self.cache.set(&self.source_name, identifier, "versions", &versions);
        
        Ok(versions)
    }
    
    async fn check_update(&self, identifier: &str, current_version: &str) -> Result<UpdateInfo> {
        // For check_update, we don't cache as it depends on current_version
        self.inner.check_update(identifier, current_version).await
    }
    
    async fn get_metadata(&self, identifier: &str, version: &str) -> Result<HashMap<String, serde_json::Value>> {
        // Create a composite key for version-specific metadata
        let cache_key = format!("{}@{}", identifier, version);
        
        // Check cache first
        if let Some(metadata) = self.cache.get::<HashMap<String, serde_json::Value>>(&self.source_name, &cache_key, "metadata") {
            return Ok(metadata);
        }
        
        // Fetch from source
        let metadata = self.inner.get_metadata(identifier, version).await?;
        
        // Cache the result
        let _ = self.cache.set(&self.source_name, &cache_key, "metadata", &metadata);
        
        Ok(metadata)
    }
}