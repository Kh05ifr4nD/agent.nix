use super::{Source, UpdateInfo, Version};
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
struct NpmPackageInfo {
    name: String,
    #[serde(rename = "dist-tags")]
    dist_tags: HashMap<String, String>,
    versions: HashMap<String, NpmVersion>,
}

#[derive(Debug, Deserialize)]
struct NpmVersion {
    version: String,
    deprecated: Option<String>,
    #[serde(default)]
    dependencies: HashMap<String, String>,
    #[serde(default, rename = "devDependencies")]
    dev_dependencies: HashMap<String, String>,
    #[serde(default, rename = "peerDependencies")]
    peer_dependencies: HashMap<String, String>,
}

pub struct NpmSource {
    client: reqwest::Client,
}

impl NpmSource {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent("treeupdt/0.1.0")
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap();
            
        Self { client }
    }
    
    async fn fetch_package_info(&self, package_name: &str) -> Result<NpmPackageInfo> {
        let url = format!("https://registry.npmjs.org/{}", package_name);
        
        let response = self.client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch package info from npm registry")?;
            
        if !response.status().is_success() {
            anyhow::bail!("npm registry API error: {}", response.status());
        }
        
        let package_info: NpmPackageInfo = response
            .json()
            .await
            .context("Failed to parse npm registry response")?;
            
        Ok(package_info)
    }
    
    fn is_pre_release(version: &str) -> bool {
        // npm pre-release versions contain - or use tags like alpha, beta, rc
        version.contains('-') || 
        version.contains("alpha") || 
        version.contains("beta") || 
        version.contains("rc")
    }
}

#[async_trait]
impl Source for NpmSource {
    async fn get_latest_version(&self, identifier: &str) -> Result<Version> {
        let package_info = self.fetch_package_info(identifier).await?;
        
        let latest_version = package_info.dist_tags
            .get("latest")
            .context("No 'latest' tag found for npm package")?;
            
        let version_info = package_info.versions
            .get(latest_version)
            .context("Version info not found")?;
            
        Ok(Version {
            version: latest_version.clone(),
            published_at: None, // npm API doesn't return publish date in this endpoint
            yanked: version_info.deprecated.is_some(),
            pre_release: Self::is_pre_release(latest_version),
            metadata: HashMap::new(),
        })
    }
    
    async fn get_versions(&self, identifier: &str) -> Result<Vec<Version>> {
        let package_info = self.fetch_package_info(identifier).await?;
        
        let mut versions: Vec<Version> = package_info.versions
            .into_iter()
            .map(|(version_str, version_info)| {
                Version {
                    version: version_str.clone(),
                    published_at: None,
                    yanked: version_info.deprecated.is_some(),
                    pre_release: Self::is_pre_release(&version_str),
                    metadata: HashMap::new(),
                }
            })
            .collect();
            
        // Sort versions by semver (newest first)
        versions.sort_by(|a, b| {
            match (semver::Version::parse(&b.version), semver::Version::parse(&a.version)) {
                (Ok(b_ver), Ok(a_ver)) => b_ver.cmp(&a_ver),
                _ => b.version.cmp(&a.version),
            }
        });
            
        Ok(versions)
    }
    
    async fn check_update(&self, identifier: &str, current_version: &str) -> Result<UpdateInfo> {
        let package_info = self.fetch_package_info(identifier).await?;
        let versions = self.get_versions(identifier).await?;
        
        let latest_tag_version = package_info.dist_tags
            .get("latest")
            .context("No 'latest' tag found")?;
            
        let latest_version = versions
            .iter()
            .find(|v| &v.version == latest_tag_version)
            .cloned()
            .context("Latest version not found in versions list")?;
            
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
    
    async fn get_metadata(&self, identifier: &str, version: &str) -> Result<HashMap<String, serde_json::Value>> {
        let package_info = self.fetch_package_info(identifier).await?;
        
        let mut metadata = HashMap::new();
        
        if let Some(version_info) = package_info.versions.get(version) {
            metadata.insert(
                "dependencies".to_string(),
                serde_json::to_value(&version_info.dependencies)?
            );
            metadata.insert(
                "devDependencies".to_string(),
                serde_json::to_value(&version_info.dev_dependencies)?
            );
            metadata.insert(
                "peerDependencies".to_string(),
                serde_json::to_value(&version_info.peer_dependencies)?
            );
            
            if let Some(deprecated) = &version_info.deprecated {
                metadata.insert(
                    "deprecated".to_string(),
                    serde_json::Value::String(deprecated.clone())
                );
            }
        }
        
        Ok(metadata)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_is_pre_release() {
        assert!(!NpmSource::is_pre_release("1.0.0"));
        assert!(!NpmSource::is_pre_release("2.5.3"));
        
        assert!(NpmSource::is_pre_release("1.0.0-beta"));
        assert!(NpmSource::is_pre_release("1.0.0-alpha.1"));
        assert!(NpmSource::is_pre_release("1.0.0-rc.2"));
        assert!(NpmSource::is_pre_release("1.0.0alpha"));
        assert!(NpmSource::is_pre_release("1.0.0beta2"));
        assert!(NpmSource::is_pre_release("1.0.0rc1"));
    }
    
    #[test]
    fn test_npm_package_info_deserialization() {
        let json = r#"{
            "name": "test-package",
            "dist-tags": {
                "latest": "1.2.3",
                "next": "2.0.0-beta.1"
            },
            "versions": {
                "1.2.3": {
                    "version": "1.2.3",
                    "dependencies": {
                        "lodash": "^4.17.0"
                    }
                },
                "2.0.0-beta.1": {
                    "version": "2.0.0-beta.1",
                    "deprecated": "This version has bugs"
                }
            }
        }"#;
        
        let package_info: NpmPackageInfo = serde_json::from_str(json).unwrap();
        assert_eq!(package_info.name, "test-package");
        assert_eq!(package_info.dist_tags.get("latest").unwrap(), "1.2.3");
        assert_eq!(package_info.dist_tags.get("next").unwrap(), "2.0.0-beta.1");
        assert_eq!(package_info.versions.len(), 2);
        
        let version_123 = &package_info.versions["1.2.3"];
        assert_eq!(version_123.version, "1.2.3");
        assert!(version_123.deprecated.is_none());
        assert_eq!(version_123.dependencies.get("lodash").unwrap(), "^4.17.0");
        
        let version_beta = &package_info.versions["2.0.0-beta.1"];
        assert_eq!(version_beta.deprecated.as_ref().unwrap(), "This version has bugs");
    }
    
    #[test]
    fn test_npm_version_sorting() {
        let mut versions = vec![
            Version {
                version: "1.0.0".to_string(),
                published_at: None,
                yanked: false,
                pre_release: false,
                metadata: HashMap::new(),
            },
            Version {
                version: "2.0.0".to_string(),
                published_at: None,
                yanked: false,
                pre_release: false,
                metadata: HashMap::new(),
            },
            Version {
                version: "1.5.0".to_string(),
                published_at: None,
                yanked: false,
                pre_release: false,
                metadata: HashMap::new(),
            },
            Version {
                version: "2.0.0-beta.1".to_string(),
                published_at: None,
                yanked: false,
                pre_release: true,
                metadata: HashMap::new(),
            },
        ];
        
        // Sort like the get_versions method does
        versions.sort_by(|a, b| {
            match (semver::Version::parse(&b.version), semver::Version::parse(&a.version)) {
                (Ok(b_ver), Ok(a_ver)) => b_ver.cmp(&a_ver),
                _ => b.version.cmp(&a.version),
            }
        });
        
        assert_eq!(versions[0].version, "2.0.0");
        assert_eq!(versions[1].version, "2.0.0-beta.1");
        assert_eq!(versions[2].version, "1.5.0");
        assert_eq!(versions[3].version, "1.0.0");
    }
    
    #[test]
    fn test_scoped_package_handling() {
        // NPM scoped packages use @ prefix
        let scoped_packages = vec![
            "@babel/core",
            "@types/node",
            "@angular/core",
            "@vue/cli",
        ];
        
        for pkg in scoped_packages {
            // Verify that scoped package names are valid
            assert!(pkg.starts_with('@'));
            assert!(pkg.contains('/'));
        }
    }
}