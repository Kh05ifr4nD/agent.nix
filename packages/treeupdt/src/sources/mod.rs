use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod github;
pub mod crates_io;
pub mod npm;
pub mod git;

pub use github::GitHubSource;
pub use crates_io::CratesIoSource;
pub use npm::NpmSource;
pub use git::GitSource;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Version {
    pub version: String,
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,
    pub yanked: bool,
    pub pre_release: bool,
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Version {
    pub fn new(version: String) -> Self {
        Self {
            version,
            published_at: None,
            yanked: false,
            pre_release: false,
            metadata: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: Version,
    pub latest_stable_version: Option<Version>,
    pub all_versions: Vec<Version>,
    pub update_available: bool,
}

#[async_trait]
pub trait Source: Send + Sync {
    /// Get the latest version information for a package
    async fn get_latest_version(&self, identifier: &str) -> Result<Version>;
    
    /// Get all available versions for a package
    async fn get_versions(&self, identifier: &str) -> Result<Vec<Version>>;
    
    /// Check if an update is available
    async fn check_update(&self, identifier: &str, current_version: &str) -> Result<UpdateInfo>;
    
    /// Get source-specific metadata
    async fn get_metadata(&self, identifier: &str, version: &str) -> Result<HashMap<String, serde_json::Value>>;
}

/// Registry of sources
pub struct SourceRegistry {
    sources: HashMap<crate::types::SourceType, Box<dyn Source>>,
}

impl SourceRegistry {
    pub fn new() -> Self {
        Self::with_cache(true)
    }
    
    pub fn with_cache(use_cache: bool) -> Self {
        let mut sources: HashMap<crate::types::SourceType, Box<dyn Source>> = HashMap::new();
        
        if use_cache {
            // Wrap sources with cache
            use crate::cache::CachedSource;
            use std::time::Duration;
            
            sources.insert(
                crate::types::SourceType::GitHub,
                Box::new(
                    CachedSource::new(GitHubSource::new(), "github".to_string())
                        .unwrap()
                        .with_ttl(Duration::from_secs(3600)) // 1 hour cache
                )
            );
            sources.insert(
                crate::types::SourceType::Crates,
                Box::new(
                    CachedSource::new(CratesIoSource::new(), "crates_io".to_string())
                        .unwrap()
                        .with_ttl(Duration::from_secs(1800)) // 30 min cache
                )
            );
            sources.insert(
                crate::types::SourceType::Npm,
                Box::new(
                    CachedSource::new(NpmSource::new(), "npm".to_string())
                        .unwrap()
                        .with_ttl(Duration::from_secs(1800)) // 30 min cache
                )
            );
            sources.insert(
                crate::types::SourceType::Git,
                Box::new(
                    CachedSource::new(GitSource::new(), "git".to_string())
                        .unwrap()
                        .with_ttl(Duration::from_secs(300)) // 5 min cache for git
                )
            );
        } else {
            // Direct sources without cache
            sources.insert(crate::types::SourceType::GitHub, Box::new(GitHubSource::new()));
            sources.insert(crate::types::SourceType::Crates, Box::new(CratesIoSource::new()));
            sources.insert(crate::types::SourceType::Npm, Box::new(NpmSource::new()));
            sources.insert(crate::types::SourceType::Git, Box::new(GitSource::new()));
        }
        
        Self { sources }
    }
    
    pub fn get_source(&self, source_type: &crate::types::SourceType) -> Option<&dyn Source> {
        self.sources.get(source_type).map(|s| s.as_ref())
    }
}