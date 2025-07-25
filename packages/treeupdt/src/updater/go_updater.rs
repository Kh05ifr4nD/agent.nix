use super::Updater;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

use crate::types::Package;

pub struct GoUpdater;

impl GoUpdater {
    pub fn new() -> Self {
        Self
    }
    
    fn update_content(&self, content: &str, package: &Package, new_version: &str) -> Result<String> {
        let mut result = String::new();
        let mut in_require_block = false;
        
        for line in content.lines() {
            let trimmed = line.trim();
            
            if trimmed == "require (" {
                in_require_block = true;
                result.push_str(line);
                result.push('\n');
            } else if in_require_block && trimmed == ")" {
                in_require_block = false;
                result.push_str(line);
                result.push('\n');
            } else if in_require_block || trimmed.starts_with("require ") {
                // Check if this is the package we want to update
                if let Some(updated_line) = self.update_require_line(line, package, new_version) {
                    result.push_str(&updated_line);
                } else {
                    result.push_str(line);
                }
                result.push('\n');
            } else if trimmed.starts_with("replace ") {
                // Handle replace directives
                if let Some(updated_line) = self.update_replace_line(line, package, new_version) {
                    result.push_str(&updated_line);
                } else {
                    result.push_str(line);
                }
                result.push('\n');
            } else {
                result.push_str(line);
                result.push('\n');
            }
        }
        
        // Remove trailing newline to match original
        if result.ends_with('\n') {
            result.pop();
        }
        
        Ok(result)
    }
    
    fn update_require_line(&self, line: &str, package: &Package, new_version: &str) -> Option<String> {
        let package_name = package.name.strip_prefix("module-")?;
        
        // Check for comment
        let (line_without_comment, comment) = if let Some(comment_idx) = line.find("//") {
            (&line[..comment_idx], Some(&line[comment_idx..]))
        } else {
            (line, None)
        };
        
        // Parse require line: module_path version
        let parts: Vec<&str> = line_without_comment.split_whitespace().collect();
        if parts.len() >= 2 && parts[0] == package_name {
            // Preserve indentation
            let indent = line.chars().take_while(|c| c.is_whitespace()).collect::<String>();
            let base = format!("{}{} {}", indent, package_name, new_version);
            if let Some(comment) = comment {
                Some(format!("{} {}", base, comment))
            } else {
                Some(base)
            }
        } else if parts.len() >= 3 && parts[0] == "require" && parts[1] == package_name {
            // Single line require
            let base = format!("require {} {}", package_name, new_version);
            if let Some(comment) = comment {
                Some(format!("{} {}", base, comment))
            } else {
                Some(base)
            }
        } else {
            None
        }
    }
    
    fn update_replace_line(&self, line: &str, package: &Package, new_version: &str) -> Option<String> {
        let package_name = package.name.strip_prefix("replace-")?;
        
        // Parse replace line: replace module_path => replacement_path version
        if line.contains(&package_name) && line.contains("=>") {
            let parts: Vec<&str> = line.split("=>").collect();
            if parts.len() == 2 {
                let replacement_parts: Vec<&str> = parts[1].trim().split_whitespace().collect();
                if replacement_parts.len() >= 2 {
                    let new_replacement = format!("{} {}", replacement_parts[0], new_version);
                    Some(format!("{} => {}", parts[0].trim_end(), new_replacement))
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    }
}

impl Updater for GoUpdater {
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
            path: "go.mod".to_string(),
            file_type: FileType::GoMod,
            name: name.to_string(),
            current_version: version.to_string(),
            sources: vec![SourceHint {
                source_type: SourceType::GitHub,
                identifier: "test/repo".to_string(),
                url: None,
            }],
            update_strategy: UpdateStrategy::Stable,
            annotations: vec![],
            metadata: HashMap::new(),
        }
    }
    
    #[test]
    fn test_update_simple_require() {
        let updater = GoUpdater::new();
        let content = r#"module example.com/mymodule

go 1.20

require github.com/stretchr/testify v1.8.0
"#;
        
        let package = create_test_package("module-github.com/stretchr/testify", "v1.8.0");
        let result = updater.update_content(content, &package, "v1.9.0").unwrap();
        
        assert!(result.contains("github.com/stretchr/testify v1.9.0"));
        assert!(!result.contains("v1.8.0"));
    }
    
    #[test]
    fn test_update_require_block() {
        let updater = GoUpdater::new();
        let content = r#"module example.com/mymodule

go 1.20

require (
	github.com/gin-gonic/gin v1.9.0
	github.com/stretchr/testify v1.8.0
	github.com/spf13/cobra v1.7.0
)
"#;
        
        let package = create_test_package("module-github.com/stretchr/testify", "v1.8.0");
        let result = updater.update_content(content, &package, "v1.9.0").unwrap();
        
        assert!(result.contains("github.com/stretchr/testify v1.9.0"));
        assert!(result.contains("github.com/gin-gonic/gin v1.9.0")); // unchanged
        assert!(result.contains("github.com/spf13/cobra v1.7.0")); // unchanged
        assert!(!result.contains("github.com/stretchr/testify v1.8.0"));
    }
    
    #[test]
    fn test_update_with_replace_directive() {
        let updater = GoUpdater::new();
        let content = r#"module example.com/mymodule

go 1.20

require github.com/old/module v1.0.0

replace github.com/old/module => github.com/new/module v2.0.0
"#;
        
        let package = create_test_package("replace-github.com/old/module", "v2.0.0");
        let result = updater.update_content(content, &package, "v2.1.0").unwrap();
        
        assert!(result.contains("replace github.com/old/module => github.com/new/module v2.1.0"));
        assert!(!result.contains("v2.0.0"));
    }
    
    #[test]
    fn test_preserve_formatting() {
        let updater = GoUpdater::new();
        let content = r#"module example.com/mymodule

go 1.20

require (
	github.com/pkg/errors v0.9.1
	golang.org/x/sync v0.3.0  // indirect
)
"#;
        
        let package = create_test_package("module-github.com/pkg/errors", "v0.9.1");
        let result = updater.update_content(content, &package, "v0.10.0").unwrap();
        
        // Check that indentation is preserved
        assert!(result.contains("\tgithub.com/pkg/errors v0.10.0"));
        // Check that comments are preserved
        assert!(result.contains("// indirect"));
    }
    
    #[test]
    fn test_update_multiple_versions() {
        let updater = GoUpdater::new();
        let content = r#"module example.com/mymodule

go 1.20

require (
	github.com/stretchr/testify v1.8.0
	github.com/stretchr/testify/assert v1.8.0
)
"#;
        
        let package = create_test_package("module-github.com/stretchr/testify", "v1.8.0");
        let result = updater.update_content(content, &package, "v1.9.0").unwrap();
        
        // Should only update exact matches
        assert!(result.contains("github.com/stretchr/testify v1.9.0"));
        // Should not update partial matches
        assert!(result.contains("github.com/stretchr/testify/assert v1.8.0"));
    }
    
    #[test]
    fn test_no_update_when_package_not_found() {
        let updater = GoUpdater::new();
        let content = r#"module example.com/mymodule

go 1.20

require github.com/other/module v1.0.0
"#;
        
        let package = create_test_package("module-github.com/not/found", "v1.0.0");
        let result = updater.update_content(content, &package, "v2.0.0").unwrap();
        
        // Content should remain unchanged
        assert_eq!(result, content.trim_end());
    }
    
    #[test]
    fn test_update_with_spaces() {
        let updater = GoUpdater::new();
        let content = r#"module example.com/mymodule

go 1.20

require   github.com/pkg/errors    v0.9.1
"#;
        
        let package = create_test_package("module-github.com/pkg/errors", "v0.9.1");
        let result = updater.update_content(content, &package, "v0.10.0").unwrap();
        
        assert!(result.contains("github.com/pkg/errors v0.10.0"));
    }
    
    #[test]
    fn test_update_indirect_dependency() {
        let updater = GoUpdater::new();
        let content = r#"module example.com/mymodule

go 1.20

require (
	github.com/direct/dep v1.0.0
	github.com/indirect/dep v2.0.0 // indirect
)
"#;
        
        let package = create_test_package("module-github.com/indirect/dep", "v2.0.0");
        let result = updater.update_content(content, &package, "v2.1.0").unwrap();
        
        assert!(result.contains("github.com/indirect/dep v2.1.0 // indirect"));
    }
}