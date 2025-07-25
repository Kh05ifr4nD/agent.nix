use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use crate::types::{UpdateStrategy, SourceType};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Global configuration
    #[serde(default)]
    pub global: GlobalConfig,
    
    /// Per-file configuration
    #[serde(default)]
    pub files: HashMap<String, FileConfig>,
    
    /// Per-package configuration
    #[serde(default)]
    pub packages: HashMap<String, PackageConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct GlobalConfig {
    /// Default update strategy
    #[serde(default = "default_update_strategy")]
    pub update_strategy: UpdateStrategy,
    
    /// Enable caching
    #[serde(default = "default_cache_enabled")]
    pub cache_enabled: bool,
    
    /// Cache TTL in seconds
    #[serde(default = "default_cache_ttl")]
    pub cache_ttl: u64,
    
    /// Default filters
    #[serde(default)]
    pub filters: FilterConfig,
    
    /// Excluded paths
    #[serde(default)]
    pub exclude_paths: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct FilterConfig {
    /// Filter by file types
    pub file_types: Option<Vec<String>>,
    
    /// Filter by package name patterns
    pub name_patterns: Option<Vec<String>>,
    
    /// Filter by source types
    pub source_types: Option<Vec<String>>,
    
    /// Filter by update strategies
    pub update_strategies: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct FileConfig {
    /// Whether this file is enabled for updates
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    
    /// Override update strategy for this file
    pub update_strategy: Option<UpdateStrategy>,
    
    /// Package-specific overrides within this file
    #[serde(default)]
    pub packages: HashMap<String, PackageConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PackageConfig {
    /// Whether this package is enabled for updates
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    
    /// Override update strategy
    pub update_strategy: Option<UpdateStrategy>,
    
    /// Pin to a specific version
    pub pin_version: Option<String>,
    
    /// Preferred source type
    pub preferred_source: Option<SourceType>,
    
    /// Ignore updates matching these patterns
    #[serde(default)]
    pub ignore_versions: Vec<String>,
}

// Default value functions
fn default_update_strategy() -> UpdateStrategy {
    UpdateStrategy::Stable
}

fn default_cache_enabled() -> bool {
    true
}

fn default_cache_ttl() -> u64 {
    3600 // 1 hour
}

fn default_enabled() -> bool {
    true
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            update_strategy: default_update_strategy(),
            cache_enabled: default_cache_enabled(),
            cache_ttl: default_cache_ttl(),
            filters: FilterConfig::default(),
            exclude_paths: Vec::new(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            global: GlobalConfig::default(),
            files: HashMap::new(),
            packages: HashMap::new(),
        }
    }
}

impl Config {
    /// Load config from a file
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }
    
    /// Load config from the default locations
    pub fn load_default() -> Result<Self> {
        // Try loading from .treeupdt.toml in current directory
        if let Ok(config) = Self::load(".treeupdt.toml") {
            return Ok(config);
        }
        
        // Try loading from treeupdt.toml in current directory
        if let Ok(config) = Self::load("treeupdt.toml") {
            return Ok(config);
        }
        
        // Try loading from user config directory
        if let Some(config_dir) = dirs::config_dir() {
            let config_path = config_dir.join("treeupdt").join("config.toml");
            if let Ok(config) = Self::load(&config_path) {
                return Ok(config);
            }
        }
        
        // Return default config if no config file found
        Ok(Self::default())
    }
    
    /// Save config to a file
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
    
    /// Get configuration for a specific file
    pub fn get_file_config(&self, path: &str) -> Option<&FileConfig> {
        // Try exact match first
        if let Some(config) = self.files.get(path) {
            return Some(config);
        }
        
        // Try without leading ./
        let normalized = path.strip_prefix("./").unwrap_or(path);
        if let Some(config) = self.files.get(normalized) {
            return Some(config);
        }
        
        // Try with leading ./
        let with_dot = format!("./{}", normalized);
        self.files.get(&with_dot)
    }
    
    /// Get configuration for a specific package
    pub fn get_package_config(&self, package_name: &str) -> Option<&PackageConfig> {
        // First check global package config
        if let Some(config) = self.packages.get(package_name) {
            return Some(config);
        }
        
        // Check if there's a file-specific config
        // This would need to be called with context about which file the package is in
        None
    }
    
    /// Check if a path should be excluded
    pub fn is_excluded(&self, path: &str) -> bool {
        self.global.exclude_paths.iter().any(|pattern| {
            // Simple glob matching - could be enhanced with proper glob library
            if pattern.contains('*') {
                // Very basic glob support
                let parts: Vec<&str> = pattern.split('*').collect();
                if parts.len() == 2 {
                    path.starts_with(parts[0]) && path.ends_with(parts[1])
                } else {
                    false
                }
            } else {
                path == pattern || path.starts_with(&format!("{}/", pattern))
            }
        })
    }
}

/// Example configuration file content
pub const EXAMPLE_CONFIG: &str = r#"# treeupdt configuration file

[global]
# Default update strategy: stable, conservative, latest, aggressive
update-strategy = "stable"

# Enable caching of API responses
cache-enabled = true

# Cache TTL in seconds (3600 = 1 hour)
cache-ttl = 3600

# Global filters
[global.filters]
# Filter by file types (nix, cargo, npm, go)
# file-types = ["nix", "cargo"]

# Filter by package name patterns (regex)
# name-patterns = ["^my-.*", ".*-internal$"]

# Filter by source types (github, npm, crates, git)
# source-types = ["github", "crates"]

# Paths to exclude from scanning
# exclude-paths = ["vendor", "node_modules", ".git"]

# Per-file configuration
[files."flake.nix"]
enabled = true
update-strategy = "conservative"

# Package-specific config within this file
[files."flake.nix".packages]
nixpkgs = { update-strategy = "stable" }
blueprint = { enabled = false }  # Don't update this input

# Global package configuration (applies across all files)
[packages]
# Pin a specific package to a version
my-important-lib = { pin-version = "1.2.3" }

# Ignore certain versions
my-package = { ignore-versions = ["*-beta*", "*-rc*"] }

# Use a specific source
some-package = { preferred-source = "github" }
"#;

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_example_config() {
        let config: Config = toml::from_str(EXAMPLE_CONFIG).unwrap();
        
        assert_eq!(config.global.update_strategy, UpdateStrategy::Stable);
        assert!(config.global.cache_enabled);
        assert_eq!(config.global.cache_ttl, 3600);
        
        // Check file config
        let flake_config = config.get_file_config("flake.nix").unwrap();
        assert!(flake_config.enabled);
        assert_eq!(flake_config.update_strategy, Some(UpdateStrategy::Conservative));
        
        // Check package config within file
        let nixpkgs_config = &flake_config.packages["nixpkgs"];
        assert_eq!(nixpkgs_config.update_strategy, Some(UpdateStrategy::Stable));
        
        let blueprint_config = &flake_config.packages["blueprint"];
        assert!(!blueprint_config.enabled);
    }
    
    #[test]
    fn test_is_excluded() {
        let mut config = Config::default();
        config.global.exclude_paths = vec![
            "vendor".to_string(),
            "node_modules".to_string(),
            "*.tmp".to_string(),
            "test_*_data".to_string(),
        ];
        
        assert!(config.is_excluded("vendor"));
        assert!(config.is_excluded("vendor/lib"));
        assert!(config.is_excluded("node_modules"));
        assert!(config.is_excluded("file.tmp"));
        assert!(config.is_excluded("test_foo_data"));
        assert!(!config.is_excluded("src"));
        assert!(!config.is_excluded("test"));
    }
}