use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use toml::Value;
use walkdir::WalkDir;

use crate::types::{Annotation, FileType, Package, Scanner, SourceHint, SourceType, UpdateStrategy};
use super::annotation_parser::extract_annotation_from_line;

pub struct CargoScanner;

impl CargoScanner {
    pub fn new() -> Self {
        Self
    }
    
    fn find_dependency_line(&self, lines: &[&str], dep_name: &str, section: &str) -> Option<usize> {
        let mut in_section = false;
        let mut _brace_depth = 0;
        
        for (idx, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            
            // Track which section we're in
            if trimmed.starts_with(&format!("[{}]", section)) {
                in_section = true;
                continue;
            } else if trimmed.starts_with('[') && !trimmed.starts_with("[[") {
                in_section = false;
            }
            
            // Track table depth for inline tables
            if in_section {
                _brace_depth += trimmed.matches('{').count();
                _brace_depth -= trimmed.matches('}').count();
                
                // Look for the dependency
                if trimmed.starts_with(&format!("{} =", dep_name)) ||
                   trimmed.starts_with(&format!("{} =", dep_name)) ||
                   trimmed.starts_with(&format!(r#""{}" ="#, dep_name)) {
                    return Some(idx);
                }
            }
        }
        
        None
    }
    
    fn scan_file(&self, file_path: &Path) -> Result<Vec<Package>> {
        let mut packages = Vec::new();
        let content = fs::read_to_string(file_path)
            .with_context(|| format!("Failed to read file: {:?}", file_path))?;
        
        // Parse lines and annotations
        let lines: Vec<&str> = content.lines().collect();
        let mut annotations_by_line: Vec<Option<Annotation>> = Vec::new();
        
        for (idx, line) in lines.iter().enumerate() {
            annotations_by_line.push(extract_annotation_from_line(line, idx + 1));
        }
        
        let cargo_toml: Value = toml::from_str(&content)
            .with_context(|| format!("Failed to parse Cargo.toml: {:?}", file_path))?;
        
        // Extract package version if this is a workspace member or standalone package
        if let Some(package) = cargo_toml.get("package") {
            if let Some(version) = package.get("version").and_then(|v| v.as_str()) {
                if let Some(name) = package.get("name").and_then(|v| v.as_str()) {
                    // Look for annotations near [package] section
                    let mut annotations = Vec::new();
                    if let Some(line_idx) = self.find_dependency_line(&lines, "name", "package") {
                        // Check lines around the package name for annotations
                        for offset in -1..=2 {
                            let check_idx = (line_idx as i32 + offset) as usize;
                            if check_idx < annotations_by_line.len() {
                                if let Some(ann) = &annotations_by_line[check_idx] {
                                    annotations.push(ann.clone());
                                }
                            }
                        }
                    }
                    
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
                        annotations,
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
                    
                    // Find annotations for this dependency
                    let mut annotations = Vec::new();
                    if let Some(line_idx) = self.find_dependency_line(&lines, name, section) {
                        // Check the line itself first for inline comment
                        if let Some(ann) = &annotations_by_line[line_idx] {
                            annotations.push(ann.clone());
                        } else {
                            // Only check lines before if there's no inline comment
                            for offset in 1..=2 {
                                if line_idx >= offset {
                                    let check_idx = line_idx - offset;
                                    if let Some(ann) = &annotations_by_line[check_idx] {
                                        // Only take if it's a comment-only line
                                        if lines[check_idx].trim().starts_with("#") || lines[check_idx].trim().starts_with("//") {
                                            annotations.push(ann.clone());
                                            break; // Only take the first annotation found
                                        }
                                    }
                                }
                            }
                        }
                    }
                    
                    packages.push(Package {
                        path: file_path.to_string_lossy().to_string(),
                        file_type: FileType::CargoToml,
                        name: format!("{}-{}", section.trim_end_matches("-dependencies"), name),
                        current_version: version,
                        sources: vec![source],
                        update_strategy: UpdateStrategy::Stable,
                        annotations,
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
                    
                    // Find annotations
                    let mut annotations = Vec::new();
                    if let Some(line_idx) = self.find_dependency_line(&lines, name, "workspace.dependencies") {
                        // Check the line itself first for inline comment
                        if let Some(ann) = &annotations_by_line[line_idx] {
                            annotations.push(ann.clone());
                        } else {
                            // Only check lines before if there's no inline comment
                            for offset in 1..=2 {
                                if line_idx >= offset {
                                    let check_idx = line_idx - offset;
                                    if let Some(ann) = &annotations_by_line[check_idx] {
                                        // Only take if it's a comment-only line
                                        if lines[check_idx].trim().starts_with("#") || lines[check_idx].trim().starts_with("//") {
                                            annotations.push(ann.clone());
                                            break; // Only take the first annotation found
                                        }
                                    }
                                }
                            }
                        }
                    }
                    
                    packages.push(Package {
                        path: file_path.to_string_lossy().to_string(),
                        file_type: FileType::CargoToml,
                        name: format!("workspace-dependency-{}", name),
                        current_version: version,
                        sources: vec![source],
                        update_strategy: UpdateStrategy::Stable,
                        annotations,
                        metadata: Default::default(),
                    });
                }
            }
        }
        
        // Handle target-specific dependencies
        if let Some(target) = cargo_toml.get("target").and_then(|v| v.as_table()) {
            for (target_name, target_value) in target {
                if let Some(target_table) = target_value.as_table() {
                    for section in &dep_sections {
                        if let Some(deps) = target_table.get(*section).and_then(|v| v.as_table()) {
                            for (name, value) in deps {
                                let (version, source) = self.parse_dependency(name, value);
                                
                                packages.push(Package {
                                    path: file_path.to_string_lossy().to_string(),
                                    file_type: FileType::CargoToml,
                                    name: format!("target.{}.{}-{}", target_name, section.trim_end_matches("-dependencies"), name),
                                    current_version: version,
                                    sources: vec![source],
                                    update_strategy: UpdateStrategy::Stable,
                                    annotations: vec![], // Target deps are complex to annotate
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
        let (version, source_type) = match value {
            Value::String(v) => (v.clone(), SourceType::Crates),
            Value::Table(t) => {
                let version = t.get("version")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                
                // Determine source type
                let source_type = if t.contains_key("git") {
                    SourceType::Git
                } else if t.contains_key("path") {
                    SourceType::Crates // Local path, but we'll treat as crates
                } else {
                    SourceType::Crates
                };
                
                (version, source_type)
            }
            _ => ("unknown".to_string(), SourceType::Crates),
        };
        
        let source = SourceHint {
            source_type,
            identifier: name.to_string(),
            url: None,
        };
        
        (version, source)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;
    
    #[test]
    fn test_scan_simple_dependencies() {
        let scanner = CargoScanner::new();
        let temp_dir = TempDir::new().unwrap();
        let cargo_toml_path = temp_dir.path().join("Cargo.toml");
        
        let content = r#"
[package]
name = "test-package"
version = "0.1.0"

[dependencies]
serde = "1.0"
tokio = { version = "1.35", features = ["full"] }
"#;
        fs::write(&cargo_toml_path, content).unwrap();
        
        let packages = scanner.scan_file(&cargo_toml_path).unwrap();
        
        assert_eq!(packages.len(), 3); // package version + 2 deps
        
        // Check package version
        let pkg = packages.iter().find(|p| p.name == "crate-test-package").unwrap();
        assert_eq!(pkg.current_version, "0.1.0");
        
        // Check dependencies
        let serde = packages.iter().find(|p| p.name == "dependencies-serde").unwrap();
        assert_eq!(serde.current_version, "1.0");
        assert_eq!(serde.sources[0].source_type, SourceType::Crates);
        
        let tokio = packages.iter().find(|p| p.name == "dependencies-tokio").unwrap();
        assert_eq!(tokio.current_version, "1.35");
    }
    
    #[test]
    fn test_scan_dev_and_build_dependencies() {
        let scanner = CargoScanner::new();
        let temp_dir = TempDir::new().unwrap();
        let cargo_toml_path = temp_dir.path().join("Cargo.toml");
        
        let content = r#"
[package]
name = "test-package"
version = "0.1.0"

[dependencies]
serde = "1.0"

[dev-dependencies]
mockall = "0.11"
criterion = "0.5"

[build-dependencies]
cc = "1.0"
"#;
        fs::write(&cargo_toml_path, content).unwrap();
        
        let packages = scanner.scan_file(&cargo_toml_path).unwrap();
        
        assert_eq!(packages.len(), 5); // package + 4 deps
        
        let mockall = packages.iter().find(|p| p.name == "dev-mockall").unwrap();
        assert_eq!(mockall.current_version, "0.11");
        
        let criterion = packages.iter().find(|p| p.name == "dev-criterion").unwrap();
        assert_eq!(criterion.current_version, "0.5");
        
        let cc = packages.iter().find(|p| p.name == "build-cc").unwrap();
        assert_eq!(cc.current_version, "1.0");
    }
    
    #[test]
    fn test_scan_workspace_dependencies() {
        let scanner = CargoScanner::new();
        let temp_dir = TempDir::new().unwrap();
        let cargo_toml_path = temp_dir.path().join("Cargo.toml");
        
        let content = r#"
[workspace]
members = ["crate-a", "crate-b"]

[workspace.dependencies]
serde = "1.0"
tokio = { version = "1.35", features = ["full"] }
"#;
        fs::write(&cargo_toml_path, content).unwrap();
        
        let packages = scanner.scan_file(&cargo_toml_path).unwrap();
        
        assert_eq!(packages.len(), 2); // 2 workspace deps
        
        let serde = packages.iter().find(|p| p.name == "workspace-dependency-serde").unwrap();
        assert_eq!(serde.current_version, "1.0");
        
        let tokio = packages.iter().find(|p| p.name == "workspace-dependency-tokio").unwrap();
        assert_eq!(tokio.current_version, "1.35");
    }
    
    #[test]
    fn test_scan_git_dependencies() {
        let scanner = CargoScanner::new();
        let temp_dir = TempDir::new().unwrap();
        let cargo_toml_path = temp_dir.path().join("Cargo.toml");
        
        let content = r#"
[package]
name = "test-package"
version = "0.1.0"

[dependencies]
my-crate = { git = "https://github.com/user/repo", version = "0.1.0" }
local-crate = { path = "../local-crate", version = "0.2.0" }
"#;
        fs::write(&cargo_toml_path, content).unwrap();
        
        let packages = scanner.scan_file(&cargo_toml_path).unwrap();
        
        let my_crate = packages.iter().find(|p| p.name == "dependencies-my-crate").unwrap();
        assert_eq!(my_crate.current_version, "0.1.0");
        assert_eq!(my_crate.sources[0].source_type, SourceType::Git);
        
        let local_crate = packages.iter().find(|p| p.name == "dependencies-local-crate").unwrap();
        assert_eq!(local_crate.current_version, "0.2.0");
    }
    
    #[test]
    fn test_scan_with_annotations() {
        let scanner = CargoScanner::new();
        let temp_dir = TempDir::new().unwrap();
        let cargo_toml_path = temp_dir.path().join("Cargo.toml");
        
        let content = r#"
[package]
name = "test-package"
version = "0.1.0"

[dependencies]
serde = "1.0"  # treeupdt: pin-version
# treeupdt: ignore
tokio = "1.35"
reqwest = "0.11" # treeupdt: update-strategy=conservative
"#;
        fs::write(&cargo_toml_path, content).unwrap();
        
        let packages = scanner.scan_file(&cargo_toml_path).unwrap();
        
        // Check serde has pin-version annotation
        let serde = packages.iter().find(|p| p.name == "dependencies-serde").unwrap();
        // Check that we have at least one annotation with pin-version
        assert!(serde.annotations.iter().any(|a| a.options.get("pin-version") == Some(&"true".to_string())));
        
        // Check tokio has ignore annotation
        let tokio = packages.iter().find(|p| p.name == "dependencies-tokio").unwrap();
        assert!(tokio.annotations.len() > 0);
        let has_ignore = tokio.annotations.iter().any(|a| a.options.contains_key("ignore"));
        assert!(has_ignore);
        
        // Check reqwest has update-strategy annotation  
        let reqwest = packages.iter().find(|p| p.name == "dependencies-reqwest").unwrap();
        assert_eq!(reqwest.annotations.len(), 1);
        assert_eq!(reqwest.annotations[0].options.get("update-strategy").unwrap(), "conservative");
    }
    
    #[test]
    fn test_scan_target_dependencies() {
        let scanner = CargoScanner::new();
        let temp_dir = TempDir::new().unwrap();
        let cargo_toml_path = temp_dir.path().join("Cargo.toml");
        
        let content = r#"
[package]
name = "test-package"
version = "0.1.0"

[target.'cfg(windows)'.dependencies]
winapi = "0.3"

[target.'cfg(unix)'.dependencies]
libc = "0.2"
"#;
        fs::write(&cargo_toml_path, content).unwrap();
        
        let packages = scanner.scan_file(&cargo_toml_path).unwrap();
        
        let winapi = packages.iter().find(|p| p.name.contains("winapi")).unwrap();
        assert_eq!(winapi.current_version, "0.3");
        
        let libc = packages.iter().find(|p| p.name.contains("libc")).unwrap();
        assert_eq!(libc.current_version, "0.2");
    }
    
    #[test]
    fn test_scan_directory() {
        let scanner = CargoScanner::new();
        let temp_dir = TempDir::new().unwrap();
        
        // Create multiple Cargo.toml files
        let sub1 = temp_dir.path().join("crate1");
        fs::create_dir(&sub1).unwrap();
        fs::write(sub1.join("Cargo.toml"), r#"
[package]
name = "crate1"
version = "0.1.0"

[dependencies]
serde = "1.0"
"#).unwrap();
        
        let sub2 = temp_dir.path().join("crate2");
        fs::create_dir(&sub2).unwrap();
        fs::write(sub2.join("Cargo.toml"), r#"
[package]
name = "crate2"
version = "0.2.0"

[dependencies]
tokio = "1.35"
"#).unwrap();
        
        let packages = scanner.scan(temp_dir.path().to_str().unwrap()).unwrap();
        
        // Should find 2 package versions and 2 dependencies
        assert_eq!(packages.len(), 4);
        
        assert!(packages.iter().any(|p| p.name == "crate-crate1" && p.current_version == "0.1.0"));
        assert!(packages.iter().any(|p| p.name == "crate-crate2" && p.current_version == "0.2.0"));
        assert!(packages.iter().any(|p| p.name == "dependencies-serde"));
        assert!(packages.iter().any(|p| p.name == "dependencies-tokio"));
    }
    
    #[test]
    fn test_scan_empty_cargo_toml() {
        let scanner = CargoScanner::new();
        let temp_dir = TempDir::new().unwrap();
        let cargo_toml_path = temp_dir.path().join("Cargo.toml");
        
        fs::write(&cargo_toml_path, "[package]\nname = \"empty\"\nversion = \"0.1.0\"").unwrap();
        
        let packages = scanner.scan_file(&cargo_toml_path).unwrap();
        assert_eq!(packages.len(), 1); // Just the package itself
    }
    
    #[test]
    fn test_scan_malformed_dependency() {
        let scanner = CargoScanner::new();
        let temp_dir = TempDir::new().unwrap();
        let cargo_toml_path = temp_dir.path().join("Cargo.toml");
        
        let content = r#"
[package]
name = "test"
version = "0.1.0"

[dependencies]
serde = "1.0"
# This will parse as a table but without version
broken = {}
tokio = "1.35"
"#;
        fs::write(&cargo_toml_path, content).unwrap();
        
        let packages = scanner.scan_file(&cargo_toml_path).unwrap();
        
        // Should still parse valid dependencies
        assert!(packages.iter().any(|p| p.name == "dependencies-serde"));
        assert!(packages.iter().any(|p| p.name == "dependencies-tokio"));
        
        // Broken dep should have "unknown" version
        let broken = packages.iter().find(|p| p.name == "dependencies-broken").unwrap();
        assert_eq!(broken.current_version, "unknown");
    }
}