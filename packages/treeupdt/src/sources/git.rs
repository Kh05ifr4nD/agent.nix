use super::{Source, UpdateInfo, Version};
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use tokio::process::Command;

pub struct GitSource {
    // Could add authentication or other config here
}

impl GitSource {
    pub fn new() -> Self {
        Self {}
    }
    
    async fn run_git_command(&self, args: &[&str], repo_url: &str) -> Result<String> {
        let output = Command::new("git")
            .args(args)
            .arg(repo_url)
            .output()
            .await
            .context("Failed to execute git command")?;
            
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Git command failed: {}", stderr);
        }
        
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
    
    async fn get_latest_commit(&self, repo_url: &str, branch: &str) -> Result<(String, chrono::DateTime<chrono::Utc>)> {
        // Use git ls-remote to get the latest commit without cloning
        let output = self.run_git_command(
            &["ls-remote", "--heads"],
            repo_url
        ).await?;
        
        let mut commit_sha = None;
        for line in output.lines() {
            if line.ends_with(&format!("refs/heads/{}", branch)) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if !parts.is_empty() {
                    commit_sha = Some(parts[0].to_string());
                    break;
                }
            }
        }
        
        let sha = commit_sha.context("Branch not found in remote repository")?;
        
        // For now, we can't get the commit date without cloning
        // In a real implementation, we might want to use the GitHub/GitLab API
        // or perform a shallow clone to get more information
        let timestamp = chrono::Utc::now();
        
        Ok((sha, timestamp))
    }
    
    fn parse_git_identifier(identifier: &str) -> Result<(String, String)> {
        // Expected format: "repo_url#branch" or just "repo_url" (defaults to main/master)
        let parts: Vec<&str> = identifier.splitn(2, '#').collect();
        let repo_url = parts[0].to_string();
        let branch = if parts.len() > 1 {
            parts[1].to_string()
        } else {
            // Try common default branches
            "main".to_string()
        };
        
        Ok((repo_url, branch))
    }
    
    fn shorten_commit_sha(sha: &str) -> String {
        // Git convention is to show first 7 characters of SHA
        if sha.len() > 7 {
            sha[..7].to_string()
        } else {
            sha.to_string()
        }
    }
}

#[async_trait]
impl Source for GitSource {
    async fn get_latest_version(&self, identifier: &str) -> Result<Version> {
        let (repo_url, branch) = Self::parse_git_identifier(identifier)?;
        let (commit_sha, timestamp) = self.get_latest_commit(&repo_url, &branch).await?;
        
        Ok(Version {
            version: commit_sha.clone(),
            published_at: Some(timestamp),
            yanked: false,
            pre_release: false,
            metadata: {
                let mut m = HashMap::new();
                m.insert("branch".to_string(), serde_json::Value::String(branch));
                m.insert("short_sha".to_string(), serde_json::Value::String(Self::shorten_commit_sha(&commit_sha)));
                m
            },
        })
    }
    
    async fn get_versions(&self, identifier: &str) -> Result<Vec<Version>> {
        // For git sources, we typically only care about the latest commit
        // Getting all commits would require cloning the repo
        let latest = self.get_latest_version(identifier).await?;
        Ok(vec![latest])
    }
    
    async fn check_update(&self, identifier: &str, current_version: &str) -> Result<UpdateInfo> {
        let latest_version = self.get_latest_version(identifier).await?;
        
        // For git commits, we check if the SHA has changed
        let update_available = !current_version.starts_with(&latest_version.version) && 
                              !latest_version.version.starts_with(current_version);
        
        Ok(UpdateInfo {
            current_version: current_version.to_string(),
            latest_version: latest_version.clone(),
            latest_stable_version: Some(latest_version.clone()),
            all_versions: vec![latest_version],
            update_available,
        })
    }
    
    async fn get_metadata(&self, identifier: &str, version: &str) -> Result<HashMap<String, serde_json::Value>> {
        let (repo_url, branch) = Self::parse_git_identifier(identifier)?;
        
        let mut metadata = HashMap::new();
        metadata.insert("repository".to_string(), serde_json::Value::String(repo_url));
        metadata.insert("branch".to_string(), serde_json::Value::String(branch));
        metadata.insert("commit".to_string(), serde_json::Value::String(version.to_string()));
        metadata.insert("short_commit".to_string(), serde_json::Value::String(Self::shorten_commit_sha(version)));
        
        Ok(metadata)
    }
}