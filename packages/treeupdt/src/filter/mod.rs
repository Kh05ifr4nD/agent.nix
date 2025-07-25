use anyhow::Result;
use regex::Regex;
use crate::types::{Package, SourceType, UpdateStrategy};

pub struct FilterConfig {
    pub file_type: Option<String>,
    pub name_pattern: Option<String>,
    pub source_type: Option<String>,
    pub update_strategy: Option<String>,
}

pub struct Filter {
    file_type: Option<String>,
    name_regex: Option<Regex>,
    source_type: Option<SourceType>,
    update_strategy: Option<UpdateStrategy>,
}

impl Filter {
    pub fn from_config(config: FilterConfig) -> Result<Self> {
        // Compile regex if provided
        let name_regex = match config.name_pattern {
            Some(pattern) => Some(Regex::new(&pattern)?),
            None => None,
        };
        
        // Parse source type
        let source_type = match config.source_type.as_deref() {
            Some("github") => Some(SourceType::GitHub),
            Some("npm") => Some(SourceType::Npm),
            Some("crates") => Some(SourceType::Crates),
            Some("git") => Some(SourceType::Git),
            Some(other) => return Err(anyhow::anyhow!("Unknown source type: {}", other)),
            None => None,
        };
        
        // Parse update strategy
        let update_strategy = match config.update_strategy.as_deref() {
            Some("stable") => Some(UpdateStrategy::Stable),
            Some("conservative") => Some(UpdateStrategy::Conservative),
            Some("latest") => Some(UpdateStrategy::Latest),
            Some("aggressive") => Some(UpdateStrategy::Aggressive),
            Some(other) => return Err(anyhow::anyhow!("Unknown update strategy: {}", other)),
            None => None,
        };
        
        Ok(Self {
            file_type: config.file_type,
            name_regex,
            source_type,
            update_strategy,
        })
    }
    
    pub fn apply(&self, packages: Vec<Package>) -> Vec<Package> {
        packages.into_iter()
            .filter(|pkg| self.matches(pkg))
            .collect()
    }
    
    fn matches(&self, package: &Package) -> bool {
        // Check file type
        if let Some(ref file_type) = self.file_type {
            let matches_type = match file_type.as_str() {
                "nix" => package.path.ends_with(".nix"),
                "cargo" => package.path.ends_with("Cargo.toml"),
                "npm" => package.path.ends_with("package.json") || package.path.ends_with("package-lock.json"),
                "go" => package.path.ends_with("go.mod"),
                _ => false,
            };
            if !matches_type {
                return false;
            }
        }
        
        // Check name pattern
        if let Some(ref regex) = self.name_regex {
            if !regex.is_match(&package.name) {
                return false;
            }
        }
        
        // Check source type
        if let Some(ref source_type) = self.source_type {
            let has_source = package.sources.iter()
                .any(|src| &src.source_type == source_type);
            if !has_source {
                return false;
            }
        }
        
        // Check update strategy
        if let Some(ref strategy) = self.update_strategy {
            if &package.update_strategy != strategy {
                return false;
            }
        }
        
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{FileType, SourceHint, SourceType};
    
    #[test]
    fn test_filter_by_file_type() {
        let filter = Filter::from_config(FilterConfig {
            file_type: Some("nix".to_string()),
            name_pattern: None,
            source_type: None,
            update_strategy: None,
        }).unwrap();
        
        let packages = vec![
            Package {
                name: "nixpkgs".to_string(),
                path: "flake.nix".to_string(),
                file_type: FileType::Nix,
                current_version: "unstable".to_string(),
                sources: vec![],
                update_strategy: UpdateStrategy::Stable,
                annotations: vec![],
                metadata: Default::default(),
            },
            Package {
                name: "serde".to_string(),
                path: "Cargo.toml".to_string(),
                file_type: FileType::CargoToml,
                current_version: "1.0.0".to_string(),
                sources: vec![],
                update_strategy: UpdateStrategy::Stable,
                annotations: vec![],
                metadata: Default::default(),
            },
        ];
        
        let filtered = filter.apply(packages);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "nixpkgs");
    }
    
    #[test]
    fn test_filter_by_name_pattern() {
        let filter = Filter::from_config(FilterConfig {
            file_type: None,
            name_pattern: Some("^serde.*".to_string()),
            source_type: None,
            update_strategy: None,
        }).unwrap();
        
        let packages = vec![
            Package {
                name: "serde".to_string(),
                path: "Cargo.toml".to_string(),
                file_type: FileType::CargoToml,
                current_version: "1.0.0".to_string(),
                sources: vec![],
                update_strategy: UpdateStrategy::Stable,
                annotations: vec![],
                metadata: Default::default(),
            },
            Package {
                name: "serde_json".to_string(),
                path: "Cargo.toml".to_string(),
                file_type: FileType::CargoToml,
                current_version: "1.0.0".to_string(),
                sources: vec![],
                update_strategy: UpdateStrategy::Stable,
                annotations: vec![],
                metadata: Default::default(),
            },
            Package {
                name: "tokio".to_string(),
                path: "Cargo.toml".to_string(),
                file_type: FileType::CargoToml,
                current_version: "1.0.0".to_string(),
                sources: vec![],
                update_strategy: UpdateStrategy::Stable,
                annotations: vec![],
                metadata: Default::default(),
            },
        ];
        
        let filtered = filter.apply(packages);
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().all(|p| p.name.starts_with("serde")));
    }
    
    #[test]
    fn test_filter_by_source_type() {
        let filter = Filter::from_config(FilterConfig {
            file_type: None,
            name_pattern: None,
            source_type: Some("github".to_string()),
            update_strategy: None,
        }).unwrap();
        
        let packages = vec![
            Package {
                name: "nixpkgs".to_string(),
                path: "flake.nix".to_string(),
                file_type: FileType::Nix,
                current_version: "unstable".to_string(),
                sources: vec![SourceHint {
                    source_type: SourceType::GitHub,
                    identifier: "NixOS/nixpkgs".to_string(),
                    url: None,
                }],
                update_strategy: UpdateStrategy::Stable,
                annotations: vec![],
                metadata: Default::default(),
            },
            Package {
                name: "serde".to_string(),
                path: "Cargo.toml".to_string(),
                file_type: FileType::CargoToml,
                current_version: "1.0.0".to_string(),
                sources: vec![SourceHint {
                    source_type: SourceType::Crates,
                    identifier: "serde".to_string(),
                    url: None,
                }],
                update_strategy: UpdateStrategy::Stable,
                annotations: vec![],
                metadata: Default::default(),
            },
        ];
        
        let filtered = filter.apply(packages);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "nixpkgs");
    }
    
    #[test]
    fn test_combined_filters() {
        let filter = Filter::from_config(FilterConfig {
            file_type: Some("cargo".to_string()),
            name_pattern: Some("serde".to_string()),
            source_type: Some("crates".to_string()),
            update_strategy: Some("stable".to_string()),
        }).unwrap();
        
        let packages = vec![
            Package {
                name: "serde".to_string(),
                path: "Cargo.toml".to_string(),
                file_type: FileType::CargoToml,
                current_version: "1.0.0".to_string(),
                sources: vec![SourceHint {
                    source_type: SourceType::Crates,
                    identifier: "serde".to_string(),
                    url: None,
                }],
                update_strategy: UpdateStrategy::Stable,
                annotations: vec![],
                metadata: Default::default(),
            },
            Package {
                name: "serde".to_string(),
                path: "package.json".to_string(),  // Wrong file type
                file_type: FileType::PackageJson,
                current_version: "1.0.0".to_string(),
                sources: vec![SourceHint {
                    source_type: SourceType::Npm,
                    identifier: "serde".to_string(),
                    url: None,
                }],
                update_strategy: UpdateStrategy::Stable,
                annotations: vec![],
                metadata: Default::default(),
            },
            Package {
                name: "tokio".to_string(),  // Wrong name
                path: "Cargo.toml".to_string(),
                file_type: FileType::CargoToml,
                current_version: "1.0.0".to_string(),
                sources: vec![SourceHint {
                    source_type: SourceType::Crates,
                    identifier: "tokio".to_string(),
                    url: None,
                }],
                update_strategy: UpdateStrategy::Stable,
                annotations: vec![],
                metadata: Default::default(),
            },
        ];
        
        let filtered = filter.apply(packages);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "serde");
        assert_eq!(filtered[0].path, "Cargo.toml");
    }
}