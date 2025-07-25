use super::{Source, UpdateInfo, Version};
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    name: Option<String>,
    published_at: Option<String>,
    prerelease: bool,
    draft: bool,
}

pub struct GitHubSource {
    client: reqwest::Client,
}

impl GitHubSource {
    pub fn new() -> Self {
        let mut headers = reqwest::header::HeaderMap::new();
        
        // Add GitHub token if available
        if let Ok(token) = std::env::var("GITHUB_TOKEN") {
            if let Ok(auth_value) = reqwest::header::HeaderValue::from_str(&format!("Bearer {}", token)) {
                headers.insert(reqwest::header::AUTHORIZATION, auth_value);
            }
        }
        
        let client = reqwest::Client::builder()
            .user_agent("treeupdt/0.1.0")
            .timeout(std::time::Duration::from_secs(30))
            .default_headers(headers)
            .build()
            .unwrap();
            
        Self { client }
    }
    
    async fn fetch_releases(&self, owner: &str, repo: &str) -> Result<Vec<GitHubRelease>> {
        let url = format!("https://api.github.com/repos/{}/{}/releases", owner, repo);
        
        let response = self.client
            .get(&url)
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await
            .context("Failed to fetch GitHub releases")?;
            
        if !response.status().is_success() {
            anyhow::bail!("GitHub API error: {}", response.status());
        }
        
        let releases: Vec<GitHubRelease> = response
            .json()
            .await
            .context("Failed to parse GitHub releases")?;
            
        Ok(releases)
    }
    
    fn parse_identifier(identifier: &str) -> Result<(&str, &str)> {
        let parts: Vec<&str> = identifier.split('/').collect();
        if parts.len() != 2 {
            anyhow::bail!("Invalid GitHub identifier format. Expected: owner/repo");
        }
        Ok((parts[0], parts[1]))
    }
    
    fn clean_version(tag: &str) -> String {
        // Remove common prefixes like 'v', 'version-', etc.
        // Check longer prefixes first
        if let Some(stripped) = tag.strip_prefix("version-") {
            return stripped.to_string();
        }
        if let Some(stripped) = tag.strip_prefix("release-") {
            return stripped.to_string();
        }
        if let Some(stripped) = tag.strip_prefix('v') {
            return stripped.to_string();
        }
        tag.to_string()
    }
}

#[async_trait]
impl Source for GitHubSource {
    async fn get_latest_version(&self, identifier: &str) -> Result<Version> {
        let (owner, repo) = Self::parse_identifier(identifier)?;
        let releases = self.fetch_releases(owner, repo).await?;
        
        let release = releases
            .into_iter()
            .filter(|r| !r.draft)
            .next()
            .context("No releases found")?;
            
        let version = Self::clean_version(&release.tag_name);
        let published_at = release.published_at
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc));
            
        Ok(Version {
            version,
            published_at,
            yanked: false,
            pre_release: release.prerelease,
            metadata: HashMap::new(),
        })
    }
    
    async fn get_versions(&self, identifier: &str) -> Result<Vec<Version>> {
        let (owner, repo) = Self::parse_identifier(identifier)?;
        let releases = self.fetch_releases(owner, repo).await?;
        
        let versions: Vec<Version> = releases
            .into_iter()
            .filter(|r| !r.draft)
            .map(|release| {
                let version = Self::clean_version(&release.tag_name);
                let published_at = release.published_at
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&chrono::Utc));
                    
                Version {
                    version,
                    published_at,
                    yanked: false,
                    pre_release: release.prerelease,
                    metadata: HashMap::new(),
                }
            })
            .collect();
            
        Ok(versions)
    }
    
    async fn check_update(&self, identifier: &str, current_version: &str) -> Result<UpdateInfo> {
        let versions = self.get_versions(identifier).await?;
        
        let latest_version = versions
            .iter()
            .next()
            .cloned()
            .context("No versions found")?;
            
        let latest_stable_version = versions
            .iter()
            .filter(|v| !v.pre_release)
            .next()
            .cloned();
            
        let current_clean = Self::clean_version(current_version);
        let update_available = latest_version.version != current_clean;
        
        Ok(UpdateInfo {
            current_version: current_version.to_string(),
            latest_version,
            latest_stable_version,
            all_versions: versions,
            update_available,
        })
    }
    
    async fn get_metadata(&self, _identifier: &str, _version: &str) -> Result<HashMap<String, serde_json::Value>> {
        // Could fetch additional release metadata if needed
        Ok(HashMap::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_identifier() {
        assert!(GitHubSource::parse_identifier("owner/repo").is_ok());
        let (owner, repo) = GitHubSource::parse_identifier("rust-lang/rust").unwrap();
        assert_eq!(owner, "rust-lang");
        assert_eq!(repo, "rust");
        
        assert!(GitHubSource::parse_identifier("invalid").is_err());
        assert!(GitHubSource::parse_identifier("too/many/slashes").is_err());
        assert!(GitHubSource::parse_identifier("").is_err());
    }
    
    #[test]
    fn test_clean_version() {
        assert_eq!(GitHubSource::clean_version("v1.0.0"), "1.0.0");
        assert_eq!(GitHubSource::clean_version("version-1.0.0"), "1.0.0");
        assert_eq!(GitHubSource::clean_version("release-1.0.0"), "1.0.0");
        assert_eq!(GitHubSource::clean_version("1.0.0"), "1.0.0");
        assert_eq!(GitHubSource::clean_version("v1.0.0-beta"), "1.0.0-beta");
        assert_eq!(GitHubSource::clean_version("V1.0.0"), "V1.0.0");
        assert_eq!(GitHubSource::clean_version("v1.2.3-rc.1+build123"), "1.2.3-rc.1+build123");
    }
    
    #[test]
    fn test_github_release_deserialization() {
        let json = r#"{
            "tag_name": "v1.0.0",
            "name": "Release 1.0.0",
            "published_at": "2023-01-01T00:00:00Z",
            "prerelease": false,
            "draft": false
        }"#;
        
        let release: GitHubRelease = serde_json::from_str(json).unwrap();
        assert_eq!(release.tag_name, "v1.0.0");
        assert_eq!(release.name, Some("Release 1.0.0".to_string()));
        assert!(!release.prerelease);
        assert!(!release.draft);
    }
    
    #[test]
    fn test_github_release_minimal_deserialization() {
        // Test that optional fields work correctly
        let json = r#"{
            "tag_name": "v2.0.0",
            "prerelease": true,
            "draft": false
        }"#;
        
        let release: GitHubRelease = serde_json::from_str(json).unwrap();
        assert_eq!(release.tag_name, "v2.0.0");
        assert_eq!(release.name, None);
        assert_eq!(release.published_at, None);
        assert!(release.prerelease);
    }
    
    #[test]
    fn test_github_token_from_env() {
        // Save current env
        let original_token = std::env::var("GITHUB_TOKEN").ok();
        
        // Test with token
        std::env::set_var("GITHUB_TOKEN", "test-token-12345");
        let source = GitHubSource::new();
        // The token should be set in the client's default headers
        // We can't directly test the headers, but we can verify it doesn't panic
        drop(source);
        
        // Test without token
        std::env::remove_var("GITHUB_TOKEN");
        let source = GitHubSource::new();
        drop(source);
        
        // Restore original env
        if let Some(token) = original_token {
            std::env::set_var("GITHUB_TOKEN", token);
        }
    }
}