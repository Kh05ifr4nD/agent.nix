use super::{Source, UpdateInfo, Version};
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
struct CratesIoResponse {
    #[serde(rename = "crate")]
    crate_info: CrateInfo,
    versions: Vec<CrateVersion>,
}

#[derive(Debug, Deserialize)]
struct CrateInfo {
    id: String,
    name: String,
    max_version: String,
}

#[derive(Debug, Deserialize)]
struct CrateVersion {
    num: String,
    yanked: bool,
    created_at: String,
}

pub struct CratesIoSource {
    client: reqwest::Client,
}

impl CratesIoSource {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent("treeupdt/0.1.0")
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap();
            
        Self { client }
    }
    
    async fn fetch_crate_info(&self, crate_name: &str) -> Result<CratesIoResponse> {
        let url = format!("https://crates.io/api/v1/crates/{}", crate_name);
        
        let response = self.client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch crate info from crates.io")?;
            
        if !response.status().is_success() {
            anyhow::bail!("Crates.io API error: {}", response.status());
        }
        
        let crate_info: CratesIoResponse = response
            .json()
            .await
            .context("Failed to parse crates.io response")?;
            
        Ok(crate_info)
    }
    
    fn is_pre_release(version: &str) -> bool {
        version.contains('-')
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_is_pre_release() {
        assert!(!CratesIoSource::is_pre_release("1.0.0"));
        assert!(!CratesIoSource::is_pre_release("2.5.3"));
        assert!(!CratesIoSource::is_pre_release("0.1.0"));
        
        assert!(CratesIoSource::is_pre_release("1.0.0-alpha"));
        assert!(CratesIoSource::is_pre_release("1.0.0-beta.1"));
        assert!(CratesIoSource::is_pre_release("1.0.0-rc.2"));
        assert!(CratesIoSource::is_pre_release("1.0.0-pre"));
        assert!(CratesIoSource::is_pre_release("1.0.0-dev"));
    }
    
    #[test]
    fn test_crates_io_response_deserialization() {
        let json = r#"{
            "crate": {
                "id": "serde",
                "name": "serde",
                "max_version": "1.0.195"
            },
            "versions": [
                {
                    "num": "1.0.195",
                    "yanked": false,
                    "created_at": "2024-01-01T00:00:00Z"
                },
                {
                    "num": "1.0.194",
                    "yanked": false,
                    "created_at": "2023-12-15T00:00:00Z"
                },
                {
                    "num": "1.0.193",
                    "yanked": true,
                    "created_at": "2023-12-01T00:00:00Z"
                }
            ]
        }"#;
        
        let response: CratesIoResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.crate_info.id, "serde");
        assert_eq!(response.crate_info.name, "serde");
        assert_eq!(response.crate_info.max_version, "1.0.195");
        assert_eq!(response.versions.len(), 3);
        
        assert_eq!(response.versions[0].num, "1.0.195");
        assert!(!response.versions[0].yanked);
        
        assert_eq!(response.versions[1].num, "1.0.194");
        assert!(!response.versions[1].yanked);
        
        assert_eq!(response.versions[2].num, "1.0.193");
        assert!(response.versions[2].yanked);
    }
    
    #[test]
    fn test_crate_version_handling() {
        let versions = vec![
            CrateVersion {
                num: "1.0.0".to_string(),
                yanked: false,
                created_at: "2023-01-01T00:00:00Z".to_string(),
            },
            CrateVersion {
                num: "1.0.0-beta.1".to_string(),
                yanked: false,
                created_at: "2022-12-01T00:00:00Z".to_string(),
            },
            CrateVersion {
                num: "0.9.0".to_string(),
                yanked: true,
                created_at: "2022-11-01T00:00:00Z".to_string(),
            },
        ];
        
        // Test that we can differentiate yanked versions
        assert!(!versions[0].yanked);
        assert!(!versions[1].yanked);
        assert!(versions[2].yanked);
        
        // Test version strings
        assert_eq!(versions[0].num, "1.0.0");
        assert!(versions[1].num.contains("-beta"));
    }
    
    #[test]
    fn test_invalid_date_format() {
        // Test that invalid dates are handled gracefully
        let version = CrateVersion {
            num: "1.0.0".to_string(),
            yanked: false,
            created_at: "invalid-date".to_string(),
        };
        
        // This should not panic when parsing
        let parsed_date = chrono::DateTime::parse_from_rfc3339(&version.created_at).ok();
        assert!(parsed_date.is_none());
    }
    
    #[test]
    fn test_crate_with_hyphens() {
        // Many crates have hyphens in their names
        let json = r#"{
            "crate": {
                "id": "serde-json",
                "name": "serde-json",
                "max_version": "1.0.111"
            },
            "versions": []
        }"#;
        
        let response: CratesIoResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.crate_info.name, "serde-json");
    }
    
    #[test]
    fn test_version_with_build_metadata() {
        // Semantic versioning allows build metadata after +
        assert!(!CratesIoSource::is_pre_release("1.0.0+20130313144700"));
        assert!(!CratesIoSource::is_pre_release("1.0.0+exp.sha.5114f85"));
        assert!(CratesIoSource::is_pre_release("1.0.0-alpha+001"));
        assert!(CratesIoSource::is_pre_release("1.0.0-beta+exp.sha.5114f85"));
    }
}

#[async_trait]
impl Source for CratesIoSource {
    async fn get_latest_version(&self, identifier: &str) -> Result<Version> {
        let crate_info = self.fetch_crate_info(identifier).await?;
        
        let version_str = crate_info.crate_info.max_version.clone();
        Ok(Version {
            version: version_str.clone(),
            published_at: None,
            yanked: false,
            pre_release: Self::is_pre_release(&version_str),
            metadata: HashMap::new(),
        })
    }
    
    async fn get_versions(&self, identifier: &str) -> Result<Vec<Version>> {
        let crate_info = self.fetch_crate_info(identifier).await?;
        
        let versions: Vec<Version> = crate_info.versions
            .into_iter()
            .map(|v| {
                let published_at = chrono::DateTime::parse_from_rfc3339(&v.created_at)
                    .ok()
                    .map(|dt| dt.with_timezone(&chrono::Utc));
                    
                Version {
                    version: v.num.clone(),
                    published_at,
                    yanked: v.yanked,
                    pre_release: Self::is_pre_release(&v.num),
                    metadata: HashMap::new(),
                }
            })
            .collect();
            
        Ok(versions)
    }
    
    async fn check_update(&self, identifier: &str, current_version: &str) -> Result<UpdateInfo> {
        let crate_info = self.fetch_crate_info(identifier).await?;
        let versions = self.get_versions(identifier).await?;
        
        let latest_version = Version {
            version: crate_info.crate_info.max_version.clone(),
            published_at: None,
            yanked: false,
            pre_release: Self::is_pre_release(&crate_info.crate_info.max_version),
            metadata: HashMap::new(),
        };
        
        let latest_stable_version = versions
            .iter()
            .filter(|v| !v.yanked && !v.pre_release)
            .next()
            .cloned();
            
        let update_available = latest_version.version != current_version;
        
        Ok(UpdateInfo {
            current_version: current_version.to_string(),
            latest_version,
            latest_stable_version,
            all_versions: versions,
            update_available,
        })
    }
    
    async fn get_metadata(&self, _identifier: &str, _version: &str) -> Result<HashMap<String, serde_json::Value>> {
        // Could fetch additional metadata like dependencies, features, etc.
        Ok(HashMap::new())
    }
}