use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Package {
    /// Location in the filesystem
    pub path: String,
    pub file_type: FileType,
    
    /// Package information
    pub name: String,
    pub current_version: String,
    
    /// Update hints
    pub sources: Vec<SourceHint>,
    pub update_strategy: UpdateStrategy,
    pub annotations: Vec<Annotation>,
    
    /// Additional metadata
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum FileType {
    Nix,
    PackageJson,
    CargoToml,
    GoMod,
    Pipfile,
    Gemfile,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceHint {
    pub source_type: SourceType,
    pub identifier: String,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceType {
    GitHub,
    Npm,
    PyPi,
    Crates,
    Git,
    Url,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UpdateStrategy {
    Conservative,
    Stable,
    Latest,
    Aggressive,
}

impl Default for UpdateStrategy {
    fn default() -> Self {
        UpdateStrategy::Stable
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Annotation {
    pub line: usize,
    pub options: std::collections::HashMap<String, String>,
}

/// Scanner trait that all language-specific scanners must implement
pub trait Scanner {
    /// Scan finds all updatable packages in the given path
    fn scan(&self, path: &str) -> anyhow::Result<Vec<Package>>;
}