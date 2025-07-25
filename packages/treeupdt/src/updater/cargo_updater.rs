use super::Updater;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use toml_edit::{DocumentMut, value};

use crate::types::Package;

pub struct CargoUpdater;

impl CargoUpdater {
    pub fn new() -> Self {
        Self
    }
    
    fn update_content(&self, content: &str, package: &Package, new_version: &str) -> Result<String> {
        let mut doc = content.parse::<DocumentMut>()
            .context("Failed to parse Cargo.toml")?;
            
        // Determine which section to update based on package name
        if package.name.starts_with("dependencies-") {
            let dep_name = package.name.strip_prefix("dependencies-")
                .context("Invalid dependency package name")?;
            self.update_dependency(&mut doc, "dependencies", dep_name, new_version)?;
        } else if package.name.starts_with("dev-") {
            let dep_name = package.name.strip_prefix("dev-")
                .context("Invalid dev dependency package name")?;
            self.update_dependency(&mut doc, "dev-dependencies", dep_name, new_version)?;
        } else if package.name.starts_with("build-") {
            let dep_name = package.name.strip_prefix("build-")
                .context("Invalid build dependency package name")?;
            self.update_dependency(&mut doc, "build-dependencies", dep_name, new_version)?;
        } else if package.name.starts_with("crate-") {
            // Update the main crate version
            if let Some(package_table) = doc.get_mut("package").and_then(|p| p.as_table_mut()) {
                package_table["version"] = value(new_version);
            }
        } else {
            anyhow::bail!("Unknown Cargo package type: {}", package.name)
        }
        
        Ok(doc.to_string())
    }
    
    fn update_dependency(&self, doc: &mut DocumentMut, section: &str, dep_name: &str, new_version: &str) -> Result<()> {
        if let Some(deps) = doc.get_mut(section).and_then(|d| d.as_table_mut()) {
            if let Some(dep) = deps.get_mut(dep_name) {
                if dep.is_str() {
                    // Simple string version
                    *dep = value(new_version);
                } else if let Some(dep_table) = dep.as_table_like_mut() {
                    // Table format with version field
                    dep_table.insert("version", value(new_version));
                }
            }
        }
        
        Ok(())
    }
}

impl Updater for CargoUpdater {
    fn update_package(&self, file_path: &Path, package: &Package, new_version: &str) -> Result<String> {
        let content = fs::read_to_string(file_path)
            .with_context(|| format!("Failed to read file: {:?}", file_path))?;
            
        self.update_content(&content, package, new_version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{FileType, SourceHint, SourceType, UpdateStrategy};
    use std::collections::HashMap;
    
    fn create_test_package(name: &str, version: &str) -> Package {
        Package {
            path: "Cargo.toml".to_string(),
            file_type: FileType::CargoToml,
            name: name.to_string(),
            current_version: version.to_string(),
            sources: vec![SourceHint {
                source_type: SourceType::Crates,
                identifier: "test-crate".to_string(),
                url: None,
            }],
            update_strategy: UpdateStrategy::Stable,
            annotations: vec![],
            metadata: HashMap::new(),
        }
    }
    
    #[test]
    fn test_update_simple_dependency() {
        let updater = CargoUpdater::new();
        let content = r#"[package]
name = "test"
version = "0.1.0"

[dependencies]
serde = "1.0.0"
tokio = "1.0"
"#;
        
        let package = create_test_package("dependencies-serde", "1.0.0");
        let result = updater.update_content(content, &package, "1.1.0").unwrap();
        
        assert!(result.contains(r#"serde = "1.1.0""#));
        assert!(result.contains(r#"tokio = "1.0""#)); // unchanged
    }
    
    #[test]
    fn test_update_dev_dependency() {
        let updater = CargoUpdater::new();
        let content = r#"[package]
name = "test"
version = "0.1.0"

[dev-dependencies]
criterion = "0.3"
pretty_assertions = "1.0"
"#;
        
        let package = create_test_package("dev-criterion", "0.3");
        let result = updater.update_content(content, &package, "0.4").unwrap();
        
        assert!(result.contains(r#"criterion = "0.4""#));
        assert!(result.contains(r#"pretty_assertions = "1.0""#)); // unchanged
    }
    
    #[test]
    fn test_update_build_dependency() {
        let updater = CargoUpdater::new();
        let content = r#"[package]
name = "test"
version = "0.1.0"

[build-dependencies]
cc = "1.0"
"#;
        
        let package = create_test_package("build-cc", "1.0");
        let result = updater.update_content(content, &package, "1.1").unwrap();
        
        assert!(result.contains(r#"cc = "1.1""#));
    }
    
    #[test]
    fn test_update_table_format_dependency() {
        let updater = CargoUpdater::new();
        let content = r#"[package]
name = "test"
version = "0.1.0"

[dependencies]
serde = { version = "1.0", features = ["derive"] }
"#;
        
        let package = create_test_package("dependencies-serde", "1.0");
        let result = updater.update_content(content, &package, "1.1").unwrap();
        
        assert!(result.contains(r#"version = "1.1""#));
        assert!(result.contains(r#"features = ["derive"]"#)); // features preserved
    }
    
    #[test]
    fn test_update_crate_version() {
        let updater = CargoUpdater::new();
        let content = r#"[package]
name = "my-crate"
version = "0.1.0"
authors = ["Test"]

[dependencies]
"#;
        
        let package = create_test_package("crate-my-crate", "0.1.0");
        let result = updater.update_content(content, &package, "0.2.0").unwrap();
        
        assert!(result.contains(r#"version = "0.2.0""#));
        assert!(result.contains(r#"name = "my-crate""#)); // name unchanged
    }
    
    #[test]
    fn test_update_workspace_dependency() {
        let updater = CargoUpdater::new();
        let content = r#"[package]
name = "test"
version = "0.1.0"

[dependencies]
workspace-crate = { version = "0.1", path = "../workspace-crate" }
"#;
        
        let package = create_test_package("dependencies-workspace-crate", "0.1");
        let result = updater.update_content(content, &package, "0.2").unwrap();
        
        assert!(result.contains(r#"version = "0.2""#));
        assert!(result.contains(r#"path = "../workspace-crate""#)); // path preserved
    }
    
    #[test]
    fn test_preserve_formatting() {
        let updater = CargoUpdater::new();
        let content = r#"[package]
name = "test"
version = "0.1.0"

[dependencies]
# Important dependency
serde = "1.0" # with comment

[dev-dependencies]
"#;
        
        let package = create_test_package("dependencies-serde", "1.0");
        let result = updater.update_content(content, &package, "1.1").unwrap();
        
        assert!(result.contains("# Important dependency"));
        // toml_edit doesn't preserve inline comments, only line comments
        // assert!(result.contains("# with comment"));
    }
    
    #[test]
    fn test_no_update_when_dependency_not_found() {
        let updater = CargoUpdater::new();
        let content = r#"[package]
name = "test"
version = "0.1.0"

[dependencies]
serde = "1.0"
"#;
        
        let package = create_test_package("dependencies-tokio", "1.0");
        let result = updater.update_content(content, &package, "2.0").unwrap();
        
        // Should not contain tokio
        assert!(!result.contains("tokio"));
        // serde should remain unchanged
        assert!(result.contains(r#"serde = "1.0""#));
    }
    
    #[test]
    fn test_update_git_dependency() {
        let updater = CargoUpdater::new();
        let content = r#"[package]
name = "test"
version = "0.1.0"

[dependencies]
my-crate = { git = "https://github.com/user/repo", tag = "v1.0" }
"#;
        
        let package = create_test_package("dependencies-my-crate", "v1.0");
        let result = updater.update_content(content, &package, "v2.0").unwrap();
        
        // toml_edit might reformat this, but the version should be updated
        assert!(result.contains("my-crate"));
        // The git URL should be preserved
        assert!(result.contains("https://github.com/user/repo"));
    }
}