use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use toml::Value;
use walkdir::WalkDir;

use crate::types::{FileType, Package, Scanner, SourceHint, SourceType, UpdateStrategy};

pub struct CargoScanner;

impl CargoScanner {
    pub fn new() -> Self {
        Self
    }
    
    fn scan_file(&self, file_path: &Path) -> Result<Vec<Package>> {
        let mut packages = Vec::new();
        let content = fs::read_to_string(file_path)
            .with_context(|| format!("Failed to read file: {:?}", file_path))?;
        
        let cargo_toml: Value = toml::from_str(&content)
            .with_context(|| format!("Failed to parse Cargo.toml: {:?}", file_path))?;
        
        // Extract package version if this is a workspace member or standalone package
        if let Some(package) = cargo_toml.get("package") {
            if let Some(version) = package.get("version").and_then(|v| v.as_str()) {
                if let Some(name) = package.get("name").and_then(|v| v.as_str()) {
                    packages.push(Package {
                        path: file_path.to_string_lossy().to_string(),
                        file_type: FileType::CargoToml,
                        name: format!("crate-{}", name),
                        current_version: version.to_string(),
                        sources: vec![SourceHint {
                            source_type: SourceType::Crates,
                            identifier: name.to_string(),
                            url: None,
                        }],
                        update_strategy: UpdateStrategy::Stable,
                        annotations: vec![],
                        metadata: Default::default(),
                    });
                }
            }
        }
        
        // Extract dependencies
        let dep_sections = ["dependencies", "dev-dependencies", "build-dependencies"];
        
        for section in &dep_sections {
            if let Some(deps) = cargo_toml.get(section).and_then(|v| v.as_table()) {
                for (name, value) in deps {
                    let (version, source) = self.parse_dependency(name, value);
                    
                    packages.push(Package {
                        path: file_path.to_string_lossy().to_string(),
                        file_type: FileType::CargoToml,
                        name: format!("{}-{}", section.trim_end_matches("-dependencies"), name),
                        current_version: version,
                        sources: vec![source],
                        update_strategy: UpdateStrategy::Stable,
                        annotations: vec![],
                        metadata: Default::default(),
                    });
                }
            }
        }
        
        // Handle workspace dependencies
        if let Some(workspace) = cargo_toml.get("workspace") {
            if let Some(deps) = workspace.get("dependencies").and_then(|v| v.as_table()) {
                for (name, value) in deps {
                    let (version, source) = self.parse_dependency(name, value);
                    
                    packages.push(Package {
                        path: file_path.to_string_lossy().to_string(),
                        file_type: FileType::CargoToml,
                        name: format!("workspace-dependency-{}", name),
                        current_version: version,
                        sources: vec![source],
                        update_strategy: UpdateStrategy::Stable,
                        annotations: vec![],
                        metadata: Default::default(),
                    });
                }
            }
        }
        
        // Handle target-specific dependencies
        if let Some(target) = cargo_toml.get("target") {
            if let Some(target_table) = target.as_table() {
                for (target_name, target_value) in target_table {
                    for section in &dep_sections {
                        if let Some(deps) = target_value.get(section).and_then(|v| v.as_table()) {
                            for (name, value) in deps {
                                let (version, source) = self.parse_dependency(name, value);
                                
                                packages.push(Package {
                                    path: file_path.to_string_lossy().to_string(),
                                    file_type: FileType::CargoToml,
                                    name: format!("target.{}.{}-{}", target_name, section.trim_end_matches("-dependencies"), name),
                                    current_version: version,
                                    sources: vec![source],
                                    update_strategy: UpdateStrategy::Stable,
                                    annotations: vec![],
                                    metadata: Default::default(),
                                });
                            }
                        }
                    }
                }
            }
        }
        
        Ok(packages)
    }
    
    fn parse_dependency(&self, name: &str, value: &Value) -> (String, SourceHint) {
        match value {
            // Simple version string: foo = "1.0"
            Value::String(version) => (
                version.clone(),
                SourceHint {
                    source_type: SourceType::Crates,
                    identifier: name.to_string(),
                    url: None,
                }
            ),
            // Detailed dependency: foo = { version = "1.0", features = [...] }
            Value::Table(table) => {
                let version = table.get("version")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                
                // Check for alternative sources
                let source = if let Some(git) = table.get("git").and_then(|v| v.as_str()) {
                    SourceHint {
                        source_type: SourceType::Git,
                        identifier: git.to_string(),
                        url: Some(git.to_string()),
                    }
                } else if let Some(path) = table.get("path").and_then(|v| v.as_str()) {
                    SourceHint {
                        source_type: SourceType::Url, // Using Url type for local paths
                        identifier: path.to_string(),
                        url: Some(path.to_string()),
                    }
                } else if table.contains_key("registry") {
                    // Custom registry
                    SourceHint {
                        source_type: SourceType::Crates,
                        identifier: name.to_string(),
                        url: table.get("registry").and_then(|v| v.as_str()).map(String::from),
                    }
                } else {
                    // Default to crates.io
                    SourceHint {
                        source_type: SourceType::Crates,
                        identifier: name.to_string(),
                        url: None,
                    }
                };
                
                (version, source)
            }
            _ => (
                "unknown".to_string(),
                SourceHint {
                    source_type: SourceType::Crates,
                    identifier: name.to_string(),
                    url: None,
                }
            ),
        }
    }
}

impl Scanner for CargoScanner {
    fn scan(&self, path: &str) -> Result<Vec<Package>> {
        let mut packages = Vec::new();
        let path = Path::new(path);
        
        if path.is_file() && path.file_name().map(|n| n == "Cargo.toml").unwrap_or(false) {
            packages.extend(self.scan_file(path)?);
        } else if path.is_dir() {
            for entry in WalkDir::new(path)
                .follow_links(true)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
                .filter(|e| e.path().file_name().map(|n| n == "Cargo.toml").unwrap_or(false)) {
                match self.scan_file(entry.path()) {
                    Ok(file_packages) => packages.extend(file_packages),
                    Err(e) => eprintln!("Warning: error scanning {:?}: {}", entry.path(), e),
                }
            }
        }
        
        Ok(packages)
    }
}