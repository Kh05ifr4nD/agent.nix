use anyhow::Result;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

use crate::types::{FileType, Package, Scanner, SourceHint, SourceType, UpdateStrategy};

pub struct NpmScanner;

impl NpmScanner {
    pub fn new() -> Self {
        Self
    }
    
    fn scan_file(&self, file_path: &Path) -> Result<Vec<Package>> {
        let mut packages = Vec::new();
        let content = fs::read_to_string(file_path)?;
        
        let package_json: serde_json::Value = serde_json::from_str(&content)?;
        
        // Add dependencies
        if let Some(deps) = package_json.get("dependencies").and_then(|v| v.as_object()) {
            for (name, version) in deps {
                packages.push(Package {
                    path: file_path.to_string_lossy().to_string(),
                    file_type: FileType::PackageJson,
                    name: format!("dependency-{}", name),
                    current_version: version.as_str().unwrap_or("unknown").to_string(),
                    sources: vec![SourceHint {
                        source_type: SourceType::Npm,
                        identifier: name.to_string(),
                        url: None,
                    }],
                    update_strategy: UpdateStrategy::Stable,
                    annotations: vec![],
                    metadata: Default::default(),
                });
            }
        }
        
        // Add devDependencies
        if let Some(deps) = package_json.get("devDependencies").and_then(|v| v.as_object()) {
            for (name, version) in deps {
                packages.push(Package {
                    path: file_path.to_string_lossy().to_string(),
                    file_type: FileType::PackageJson,
                    name: format!("devDependency-{}", name),
                    current_version: version.as_str().unwrap_or("unknown").to_string(),
                    sources: vec![SourceHint {
                        source_type: SourceType::Npm,
                        identifier: name.to_string(),
                        url: None,
                    }],
                    update_strategy: UpdateStrategy::Stable,
                    annotations: vec![],
                    metadata: Default::default(),
                });
            }
        }
        
        Ok(packages)
    }
}

impl Scanner for NpmScanner {
    fn scan(&self, path: &str) -> Result<Vec<Package>> {
        let mut packages = Vec::new();
        let path = Path::new(path);
        
        if path.is_file() && path.file_name().map(|n| n == "package.json").unwrap_or(false) {
            packages.extend(self.scan_file(path)?);
        } else if path.is_dir() {
            for entry in WalkDir::new(path)
                .follow_links(true)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
                .filter(|e| e.path().file_name().map(|n| n == "package.json").unwrap_or(false))
                .filter(|e| !e.path().components().any(|c| c.as_os_str() == "node_modules")) {
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
        let scanner = NpmScanner::new();
        let temp_dir = TempDir::new().unwrap();
        let package_json_path = temp_dir.path().join("package.json");
        
        let content = r#"
{
  "name": "test-project",
  "version": "1.0.0",
  "dependencies": {
    "express": "^4.18.0",
    "lodash": "~4.17.21"
  }
}
"#;
        fs::write(&package_json_path, content).unwrap();
        
        let packages = scanner.scan_file(&package_json_path).unwrap();
        
        assert_eq!(packages.len(), 2);
        
        let express = packages.iter().find(|p| p.name == "dependency-express").unwrap();
        assert_eq!(express.current_version, "^4.18.0");
        assert_eq!(express.sources[0].source_type, SourceType::Npm);
        assert_eq!(express.sources[0].identifier, "express");
        
        let lodash = packages.iter().find(|p| p.name == "dependency-lodash").unwrap();
        assert_eq!(lodash.current_version, "~4.17.21");
    }
    
    #[test]
    fn test_scan_dev_dependencies() {
        let scanner = NpmScanner::new();
        let temp_dir = TempDir::new().unwrap();
        let package_json_path = temp_dir.path().join("package.json");
        
        let content = r#"
{
  "name": "test-project",
  "version": "1.0.0",
  "devDependencies": {
    "jest": "^29.0.0",
    "eslint": "^8.0.0",
    "typescript": "^5.0.0"
  }
}
"#;
        fs::write(&package_json_path, content).unwrap();
        
        let packages = scanner.scan_file(&package_json_path).unwrap();
        
        assert_eq!(packages.len(), 3);
        
        let jest = packages.iter().find(|p| p.name == "devDependency-jest").unwrap();
        assert_eq!(jest.current_version, "^29.0.0");
        
        let eslint = packages.iter().find(|p| p.name == "devDependency-eslint").unwrap();
        assert_eq!(eslint.current_version, "^8.0.0");
        
        let typescript = packages.iter().find(|p| p.name == "devDependency-typescript").unwrap();
        assert_eq!(typescript.current_version, "^5.0.0");
    }
    
    #[test]
    fn test_scan_mixed_dependencies() {
        let scanner = NpmScanner::new();
        let temp_dir = TempDir::new().unwrap();
        let package_json_path = temp_dir.path().join("package.json");
        
        let content = r#"
{
  "name": "test-project",
  "version": "1.0.0",
  "dependencies": {
    "react": "^18.0.0",
    "react-dom": "^18.0.0"
  },
  "devDependencies": {
    "@types/react": "^18.0.0",
    "vite": "^4.0.0"
  }
}
"#;
        fs::write(&package_json_path, content).unwrap();
        
        let packages = scanner.scan_file(&package_json_path).unwrap();
        
        assert_eq!(packages.len(), 4);
        
        // Check regular dependencies
        assert!(packages.iter().any(|p| p.name == "dependency-react"));
        assert!(packages.iter().any(|p| p.name == "dependency-react-dom"));
        
        // Check dev dependencies
        assert!(packages.iter().any(|p| p.name == "devDependency-@types/react"));
        assert!(packages.iter().any(|p| p.name == "devDependency-vite"));
    }
    
    #[test]
    fn test_scan_scoped_packages() {
        let scanner = NpmScanner::new();
        let temp_dir = TempDir::new().unwrap();
        let package_json_path = temp_dir.path().join("package.json");
        
        let content = r#"
{
  "name": "@myorg/my-package",
  "version": "1.0.0",
  "dependencies": {
    "@babel/core": "^7.0.0",
    "@babel/preset-env": "^7.0.0"
  }
}
"#;
        fs::write(&package_json_path, content).unwrap();
        
        let packages = scanner.scan_file(&package_json_path).unwrap();
        
        assert_eq!(packages.len(), 2);
        
        let babel_core = packages.iter().find(|p| p.name == "dependency-@babel/core").unwrap();
        assert_eq!(babel_core.sources[0].identifier, "@babel/core");
        
        let babel_preset = packages.iter().find(|p| p.name == "dependency-@babel/preset-env").unwrap();
        assert_eq!(babel_preset.sources[0].identifier, "@babel/preset-env");
    }
    
    #[test]
    fn test_scan_directory() {
        let scanner = NpmScanner::new();
        let temp_dir = TempDir::new().unwrap();
        
        // Create multiple package.json files
        let sub1 = temp_dir.path().join("frontend");
        fs::create_dir(&sub1).unwrap();
        fs::write(sub1.join("package.json"), r#"
{
  "name": "frontend",
  "dependencies": {
    "react": "^18.0.0"
  }
}
"#).unwrap();
        
        let sub2 = temp_dir.path().join("backend");
        fs::create_dir(&sub2).unwrap();
        fs::write(sub2.join("package.json"), r#"
{
  "name": "backend",
  "dependencies": {
    "express": "^4.0.0"
  }
}
"#).unwrap();
        
        // Create node_modules that should be ignored
        let node_modules = temp_dir.path().join("node_modules");
        fs::create_dir(&node_modules).unwrap();
        fs::write(node_modules.join("package.json"), r#"
{
  "name": "should-be-ignored",
  "dependencies": {
    "ignored": "1.0.0"
  }
}
"#).unwrap();
        
        let packages = scanner.scan(temp_dir.path().to_str().unwrap()).unwrap();
        
        // Should find 2 packages (react and express), not the one in node_modules
        assert_eq!(packages.len(), 2);
        assert!(packages.iter().any(|p| p.name == "dependency-react"));
        assert!(packages.iter().any(|p| p.name == "dependency-express"));
        assert!(!packages.iter().any(|p| p.name == "dependency-ignored"));
    }
    
    #[test]
    fn test_scan_empty_package_json() {
        let scanner = NpmScanner::new();
        let temp_dir = TempDir::new().unwrap();
        let package_json_path = temp_dir.path().join("package.json");
        
        fs::write(&package_json_path, r#"{"name": "empty"}"#).unwrap();
        
        let packages = scanner.scan_file(&package_json_path).unwrap();
        assert_eq!(packages.len(), 0);
    }
    
    #[test]
    fn test_scan_invalid_version_format() {
        let scanner = NpmScanner::new();
        let temp_dir = TempDir::new().unwrap();
        let package_json_path = temp_dir.path().join("package.json");
        
        let content = r#"
{
  "name": "test",
  "dependencies": {
    "valid": "1.0.0",
    "git-url": "git://github.com/user/repo.git",
    "file-path": "file:../local-package",
    "latest": "latest",
    "star": "*"
  }
}
"#;
        fs::write(&package_json_path, content).unwrap();
        
        let packages = scanner.scan_file(&package_json_path).unwrap();
        
        assert_eq!(packages.len(), 5);
        
        // All should be captured with their version strings
        let git_url = packages.iter().find(|p| p.name == "dependency-git-url").unwrap();
        assert_eq!(git_url.current_version, "git://github.com/user/repo.git");
        
        let latest = packages.iter().find(|p| p.name == "dependency-latest").unwrap();
        assert_eq!(latest.current_version, "latest");
    }
    
    #[test] 
    fn test_scan_workspace_packages() {
        let scanner = NpmScanner::new();
        let temp_dir = TempDir::new().unwrap();
        let package_json_path = temp_dir.path().join("package.json");
        
        let content = r#"
{
  "name": "monorepo-root",
  "workspaces": [
    "packages/*"
  ],
  "devDependencies": {
    "lerna": "^6.0.0"
  }
}
"#;
        fs::write(&package_json_path, content).unwrap();
        
        let packages = scanner.scan_file(&package_json_path).unwrap();
        
        // Should only find the lerna dependency
        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].name, "devDependency-lerna");
    }
}