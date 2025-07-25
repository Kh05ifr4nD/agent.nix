use anyhow::Result;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

use crate::types::{FileType, Package, Scanner, SourceHint, SourceType, UpdateStrategy};
use super::annotation_parser::extract_annotation_from_line;

pub struct GoModScanner;

impl GoModScanner {
    pub fn new() -> Self {
        Self
    }
    
    fn scan_file(&self, file_path: &Path) -> Result<Vec<Package>> {
        let mut packages = Vec::new();
        let content = fs::read_to_string(file_path)?;
        let lines: Vec<&str> = content.lines().collect();
        
        // Extract Go version
        if let Some(captures) = regex::Regex::new(r"(?m)^go\s+(\d+\.\d+(?:\.\d+)?)")
            .unwrap()
            .captures(&content) {
            packages.push(Package {
                path: file_path.to_string_lossy().to_string(),
                file_type: FileType::GoMod,
                name: "go-version".to_string(),
                current_version: captures.get(1).unwrap().as_str().to_string(),
                sources: vec![],
                update_strategy: UpdateStrategy::Conservative,
                annotations: vec![],
            metadata: Default::default(),
            });
        }
        
        // Extract dependencies from require blocks
        let mut in_require_block = false;
        for (line_idx, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            
            if trimmed == "require (" {
                in_require_block = true;
                continue;
            }
            
            if in_require_block && trimmed == ")" {
                in_require_block = false;
                continue;
            }
            
            // Parse require statements
            if let Some(captures) = regex::Regex::new(r"^(?:require\s+)?([^\s]+)\s+v(.+?)(?:\s+//.*)?$")
                .unwrap()
                .captures(trimmed) {
                if in_require_block || trimmed.starts_with("require ") {
                    let module = captures.get(1).unwrap().as_str();
                    let version = captures.get(2).unwrap().as_str();
                    
                    // Extract annotations from current line and nearby lines
                    let mut annotations = Vec::new();
                    
                    // Check the current line for inline comment
                    if let Some(ann) = extract_annotation_from_line(line, line_idx + 1) {
                        annotations.push(ann);
                    }
                    
                    // Only check lines before if there's no inline comment
                    if annotations.is_empty() {
                        // Check up to 2 lines before for annotations
                        for offset in 1..=2 {
                            if line_idx >= offset {
                                let prev_line = lines[line_idx - offset];
                                // Only take annotation if it's a comment-only line
                                if prev_line.trim().starts_with("//") {
                                    if let Some(ann) = extract_annotation_from_line(prev_line, line_idx - offset + 1) {
                                        annotations.push(ann);
                                        break; // Only take the first annotation found
                                    }
                                }
                            }
                        }
                    }
                    
                    packages.push(Package {
                        path: file_path.to_string_lossy().to_string(),
                        file_type: FileType::GoMod,
                        name: module.to_string(),
                        current_version: version.to_string(),
                        sources: vec![SourceHint {
                            source_type: SourceType::Git,
                            identifier: module.to_string(),
                            url: None,
                        }],
                        update_strategy: UpdateStrategy::Stable,
                        annotations,
                        metadata: Default::default(),
                    });
                }
            }
        }
        
        Ok(packages)
    }
}

impl Scanner for GoModScanner {
    fn scan(&self, path: &str) -> Result<Vec<Package>> {
        let mut packages = Vec::new();
        let path = Path::new(path);
        
        if path.is_file() && path.file_name().map(|n| n == "go.mod").unwrap_or(false) {
            packages.extend(self.scan_file(path)?);
        } else if path.is_dir() {
            for entry in WalkDir::new(path)
                .follow_links(true)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
                .filter(|e| e.path().file_name().map(|n| n == "go.mod").unwrap_or(false)) {
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
    fn test_scan_go_version() {
        let scanner = GoModScanner::new();
        let temp_dir = TempDir::new().unwrap();
        let go_mod_path = temp_dir.path().join("go.mod");
        
        let content = r#"
module example.com/myapp

go 1.21

require (
    github.com/spf13/cobra v1.7.0
)
"#;
        fs::write(&go_mod_path, content).unwrap();
        
        let packages = scanner.scan_file(&go_mod_path).unwrap();
        
        // Should find go version and one dependency
        assert_eq!(packages.len(), 2);
        
        // Check go version
        let go_version = packages.iter().find(|p| p.name == "go-version").unwrap();
        assert_eq!(go_version.current_version, "1.21");
        assert_eq!(go_version.update_strategy, UpdateStrategy::Conservative);
        
        // Check dependency
        let cobra = packages.iter().find(|p| p.name == "github.com/spf13/cobra").unwrap();
        assert_eq!(cobra.current_version, "1.7.0");
    }

    #[test]
    fn test_scan_with_require_block() {
        let scanner = GoModScanner::new();
        let temp_dir = TempDir::new().unwrap();
        let go_mod_path = temp_dir.path().join("go.mod");
        
        let content = r#"
module example.com/myapp

go 1.20

require (
    github.com/gin-gonic/gin v1.9.1
    github.com/stretchr/testify v1.8.4
    golang.org/x/tools v0.13.0
)
"#;
        fs::write(&go_mod_path, content).unwrap();
        
        let packages = scanner.scan_file(&go_mod_path).unwrap();
        
        assert_eq!(packages.len(), 4); // go version + 3 deps
        
        let gin = packages.iter().find(|p| p.name == "github.com/gin-gonic/gin").unwrap();
        assert_eq!(gin.current_version, "1.9.1");
        
        let testify = packages.iter().find(|p| p.name == "github.com/stretchr/testify").unwrap();
        assert_eq!(testify.current_version, "1.8.4");
        
        let tools = packages.iter().find(|p| p.name == "golang.org/x/tools").unwrap();
        assert_eq!(tools.current_version, "0.13.0");
    }

    #[test]
    fn test_scan_with_inline_require() {
        let scanner = GoModScanner::new();
        let temp_dir = TempDir::new().unwrap();
        let go_mod_path = temp_dir.path().join("go.mod");
        
        let content = r#"
module example.com/myapp

go 1.21

require github.com/spf13/cobra v1.7.0
require github.com/spf13/viper v1.16.0
"#;
        fs::write(&go_mod_path, content).unwrap();
        
        let packages = scanner.scan_file(&go_mod_path).unwrap();
        
        assert_eq!(packages.len(), 3); // go version + 2 deps
        
        let cobra = packages.iter().find(|p| p.name == "github.com/spf13/cobra").unwrap();
        assert_eq!(cobra.current_version, "1.7.0");
        
        let viper = packages.iter().find(|p| p.name == "github.com/spf13/viper").unwrap();
        assert_eq!(viper.current_version, "1.16.0");
    }

    #[test]
    fn test_scan_with_annotations() {
        let scanner = GoModScanner::new();
        let temp_dir = TempDir::new().unwrap();
        let go_mod_path = temp_dir.path().join("go.mod");
        
        let content = r#"
module example.com/myapp

go 1.21

require (
    github.com/spf13/cobra v1.7.0
    // treeupdt: ignore
    github.com/stretchr/testify v1.8.4
    github.com/go-chi/chi/v5 v5.0.10 // treeupdt: update-strategy=conservative
)
"#;
        fs::write(&go_mod_path, content).unwrap();
        
        let packages = scanner.scan_file(&go_mod_path).unwrap();
        
        // Check cobra has no annotations
        let cobra = packages.iter().find(|p| p.name == "github.com/spf13/cobra").unwrap();
        assert_eq!(cobra.annotations.len(), 0);
        
        // Check testify has ignore annotation
        let testify = packages.iter().find(|p| p.name == "github.com/stretchr/testify").unwrap();
        assert_eq!(testify.annotations.len(), 1);
        assert_eq!(testify.annotations[0].options.get("ignore").unwrap(), "true");
        
        // Check chi has update-strategy annotation
        let chi = packages.iter().find(|p| p.name == "github.com/go-chi/chi/v5").unwrap();
        assert_eq!(chi.annotations.len(), 1);
        assert_eq!(chi.annotations[0].options.get("update-strategy").unwrap(), "conservative");
    }

    #[test]
    fn test_scan_with_comments() {
        let scanner = GoModScanner::new();
        let temp_dir = TempDir::new().unwrap();
        let go_mod_path = temp_dir.path().join("go.mod");
        
        let content = r#"
module example.com/myapp

go 1.21

require (
    // Regular comment
    github.com/spf13/cobra v1.7.0 // indirect
    github.com/spf13/viper v1.16.0 // some comment
)
"#;
        fs::write(&go_mod_path, content).unwrap();
        
        let packages = scanner.scan_file(&go_mod_path).unwrap();
        
        // Should still parse dependencies with comments
        assert_eq!(packages.len(), 3); // go version + 2 deps
        
        let cobra = packages.iter().find(|p| p.name == "github.com/spf13/cobra").unwrap();
        assert_eq!(cobra.current_version, "1.7.0");
        
        let viper = packages.iter().find(|p| p.name == "github.com/spf13/viper").unwrap();
        assert_eq!(viper.current_version, "1.16.0");
    }

    #[test]
    fn test_scan_directory() {
        let scanner = GoModScanner::new();
        let temp_dir = TempDir::new().unwrap();
        
        // Create multiple go.mod files in subdirectories
        let sub1 = temp_dir.path().join("service1");
        fs::create_dir(&sub1).unwrap();
        fs::write(sub1.join("go.mod"), "module service1\n\ngo 1.20").unwrap();
        
        let sub2 = temp_dir.path().join("service2");
        fs::create_dir(&sub2).unwrap();
        fs::write(sub2.join("go.mod"), "module service2\n\ngo 1.21").unwrap();
        
        let packages = scanner.scan(temp_dir.path().to_str().unwrap()).unwrap();
        
        assert_eq!(packages.len(), 2); // Two go versions from two files
        
        let versions: Vec<&str> = packages.iter()
            .map(|p| p.current_version.as_str())
            .collect();
        assert!(versions.contains(&"1.20"));
        assert!(versions.contains(&"1.21"));
    }

    #[test]
    fn test_scan_empty_file() {
        let scanner = GoModScanner::new();
        let temp_dir = TempDir::new().unwrap();
        let go_mod_path = temp_dir.path().join("go.mod");
        
        fs::write(&go_mod_path, "").unwrap();
        
        let packages = scanner.scan_file(&go_mod_path).unwrap();
        assert_eq!(packages.len(), 0);
    }

    #[test]
    fn test_scan_malformed_require() {
        let scanner = GoModScanner::new();
        let temp_dir = TempDir::new().unwrap();
        let go_mod_path = temp_dir.path().join("go.mod");
        
        let content = r#"
module example.com/myapp

go 1.21

require (
    github.com/spf13/cobra v1.7.0
    malformed line without version
    github.com/spf13/viper v1.16.0
)
"#;
        fs::write(&go_mod_path, content).unwrap();
        
        let packages = scanner.scan_file(&go_mod_path).unwrap();
        
        // Should skip malformed line and parse valid ones
        assert_eq!(packages.len(), 3); // go version + 2 valid deps
    }
}