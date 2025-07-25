use super::Updater;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use serde_json::{Value, Map};

use crate::types::Package;

pub struct NpmUpdater;

impl NpmUpdater {
    pub fn new() -> Self {
        Self
    }
    
    fn update_content(&self, content: &str, package: &Package, new_version: &str) -> Result<String> {
        let mut json: Value = serde_json::from_str(content)
            .context("Failed to parse package.json")?;
            
        let obj = json.as_object_mut()
            .context("package.json is not an object")?;
            
        // Determine which section to update based on package name
        if package.name.starts_with("dependency-") {
            let dep_name = package.name.strip_prefix("dependency-")
                .context("Invalid dependency package name")?;
            self.update_dependency(obj, "dependencies", dep_name, new_version)?;
        } else if package.name.starts_with("devDependency-") {
            let dep_name = package.name.strip_prefix("devDependency-")
                .context("Invalid devDependency package name")?;
            self.update_dependency(obj, "devDependencies", dep_name, new_version)?;
        } else if package.name.starts_with("peerDependency-") {
            let dep_name = package.name.strip_prefix("peerDependency-")
                .context("Invalid peerDependency package name")?;
            self.update_dependency(obj, "peerDependencies", dep_name, new_version)?;
        } else if package.name == "package" {
            // Update the main package version
            obj.insert("version".to_string(), Value::String(new_version.to_string()));
        } else {
            anyhow::bail!("Unknown npm package type: {}", package.name)
        }
        
        // Pretty print with 2 spaces
        serde_json::to_string_pretty(&json)
            .context("Failed to serialize package.json")
    }
    
    fn update_dependency(&self, obj: &mut Map<String, Value>, section: &str, dep_name: &str, new_version: &str) -> Result<()> {
        if let Some(deps) = obj.get_mut(section).and_then(|d| d.as_object_mut()) {
            if deps.contains_key(dep_name) {
                // Preserve version prefix (^, ~, etc) if present
                if let Some(old_version) = deps.get(dep_name).and_then(|v| v.as_str()) {
                    let new_version_with_prefix = if old_version.starts_with('^') {
                        format!("^{}", new_version)
                    } else if old_version.starts_with('~') {
                        format!("~{}", new_version)
                    } else if old_version.starts_with(">=") {
                        format!(">={}", new_version)
                    } else {
                        new_version.to_string()
                    };
                    deps.insert(dep_name.to_string(), Value::String(new_version_with_prefix));
                }
            }
        }
        
        Ok(())
    }
}

impl Updater for NpmUpdater {
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
            path: "package.json".to_string(),
            file_type: FileType::PackageJson,
            name: name.to_string(),
            current_version: version.to_string(),
            sources: vec![SourceHint {
                source_type: SourceType::Npm,
                identifier: "test-package".to_string(),
                url: None,
            }],
            update_strategy: UpdateStrategy::Stable,
            annotations: vec![],
            metadata: HashMap::new(),
        }
    }
    
    #[test]
    fn test_update_simple_dependency() {
        let updater = NpmUpdater::new();
        let content = r#"{
  "name": "test-app",
  "version": "1.0.0",
  "dependencies": {
    "express": "4.18.0",
    "lodash": "4.17.21"
  }
}"#;
        
        let package = create_test_package("dependency-express", "4.18.0");
        let result = updater.update_content(content, &package, "4.19.0").unwrap();
        
        assert!(result.contains(r#""express": "4.19.0""#));
        assert!(result.contains(r#""lodash": "4.17.21""#)); // unchanged
    }
    
    #[test]
    fn test_update_dev_dependency() {
        let updater = NpmUpdater::new();
        let content = r#"{
  "name": "test-app",
  "version": "1.0.0",
  "devDependencies": {
    "jest": "29.0.0",
    "eslint": "8.0.0"
  }
}"#;
        
        let package = create_test_package("devDependency-jest", "29.0.0");
        let result = updater.update_content(content, &package, "29.1.0").unwrap();
        
        assert!(result.contains(r#""jest": "29.1.0""#));
        assert!(result.contains(r#""eslint": "8.0.0""#)); // unchanged
    }
    
    #[test]
    fn test_update_peer_dependency() {
        let updater = NpmUpdater::new();
        let content = r#"{
  "name": "test-lib",
  "version": "1.0.0",
  "peerDependencies": {
    "react": "18.0.0",
    "react-dom": "18.0.0"
  }
}"#;
        
        let package = create_test_package("peerDependency-react", "18.0.0");
        let result = updater.update_content(content, &package, "18.2.0").unwrap();
        
        assert!(result.contains(r#""react": "18.2.0""#));
        assert!(result.contains(r#""react-dom": "18.0.0""#)); // unchanged
    }
    
    #[test]
    fn test_preserve_version_prefix_caret() {
        let updater = NpmUpdater::new();
        let content = r#"{
  "name": "test-app",
  "version": "1.0.0",
  "dependencies": {
    "express": "^4.18.0"
  }
}"#;
        
        let package = create_test_package("dependency-express", "^4.18.0");
        let result = updater.update_content(content, &package, "4.19.0").unwrap();
        
        assert!(result.contains(r#""express": "^4.19.0""#));
    }
    
    #[test]
    fn test_preserve_version_prefix_tilde() {
        let updater = NpmUpdater::new();
        let content = r#"{
  "name": "test-app",
  "version": "1.0.0",
  "dependencies": {
    "express": "~4.18.0"
  }
}"#;
        
        let package = create_test_package("dependency-express", "~4.18.0");
        let result = updater.update_content(content, &package, "4.18.1").unwrap();
        
        assert!(result.contains(r#""express": "~4.18.1""#));
    }
    
    #[test]
    fn test_preserve_version_prefix_gte() {
        let updater = NpmUpdater::new();
        let content = r#"{
  "name": "test-app",
  "version": "1.0.0",
  "dependencies": {
    "express": ">=4.0.0"
  }
}"#;
        
        let package = create_test_package("dependency-express", ">=4.0.0");
        let result = updater.update_content(content, &package, "5.0.0").unwrap();
        
        assert!(result.contains(r#""express": ">=5.0.0""#));
    }
    
    #[test]
    fn test_update_package_version() {
        let updater = NpmUpdater::new();
        let content = r#"{
  "name": "my-package",
  "version": "1.0.0",
  "description": "Test package"
}"#;
        
        let package = create_test_package("package", "1.0.0");
        let result = updater.update_content(content, &package, "1.1.0").unwrap();
        
        assert!(result.contains(r#""version": "1.1.0""#));
        assert!(result.contains(r#""name": "my-package""#)); // unchanged
    }
    
    #[test]
    fn test_update_scoped_package() {
        let updater = NpmUpdater::new();
        let content = r#"{
  "name": "test-app",
  "version": "1.0.0",
  "dependencies": {
    "@babel/core": "^7.0.0",
    "@babel/preset-env": "^7.0.0"
  }
}"#;
        
        let package = create_test_package("dependency-@babel/core", "^7.0.0");
        let result = updater.update_content(content, &package, "7.1.0").unwrap();
        
        assert!(result.contains(r#""@babel/core": "^7.1.0""#));
        assert!(result.contains(r#""@babel/preset-env": "^7.0.0""#)); // unchanged
    }
    
    #[test]
    fn test_no_update_when_dependency_not_found() {
        let updater = NpmUpdater::new();
        let content = r#"{
  "name": "test-app",
  "version": "1.0.0",
  "dependencies": {
    "express": "4.18.0"
  }
}"#;
        
        let package = create_test_package("dependency-lodash", "4.17.0");
        let result = updater.update_content(content, &package, "4.18.0").unwrap();
        
        // Should not contain lodash
        assert!(!result.contains("lodash"));
        // express should remain unchanged
        assert!(result.contains(r#""express": "4.18.0""#));
    }
    
    #[test]
    fn test_preserve_formatting() {
        let updater = NpmUpdater::new();
        let content = r#"{
  "name": "test-app",
  "version": "1.0.0",
  "dependencies": {
    "express": "4.18.0"
  },
  "scripts": {
    "start": "node index.js"
  }
}"#;
        
        let package = create_test_package("dependency-express", "4.18.0");
        let result = updater.update_content(content, &package, "4.19.0").unwrap();
        
        // Check that formatting is preserved (2 space indent)
        assert!(result.contains("  \"dependencies\": {"));
        assert!(result.contains("  \"scripts\": {"));
    }
}