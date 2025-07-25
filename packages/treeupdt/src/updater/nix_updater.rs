use super::Updater;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use tree_sitter::{Parser, Query, QueryCursor};

use crate::types::Package;

pub struct NixUpdater;

impl NixUpdater {
    pub fn new() -> Self {
        Self
    }
    
    fn update_content(&self, content: &str, package: &Package, new_version: &str) -> Result<String> {
        let mut parser = Parser::new();
        let language_fn = tree_sitter_nix::LANGUAGE;
        let language = unsafe { 
            tree_sitter::Language::from_raw(language_fn.into_raw()() as *const _) 
        };
        parser.set_language(&language)?;

        let tree = parser.parse(content, None)
            .context("Failed to parse Nix file")?;

        // Handle different types of Nix packages
        if package.name.starts_with("flake-input-") {
            self.update_flake_input(content, &tree, package, new_version)
        } else if package.name == "package" {
            self.update_package_version(content, &tree, package, new_version)
        } else {
            anyhow::bail!("Unknown Nix package type: {}", package.name)
        }
    }
    
    fn update_flake_input(&self, content: &str, tree: &tree_sitter::Tree, package: &Package, new_version: &str) -> Result<String> {
        let input_name = package.name.strip_prefix("flake-input-")
            .context("Invalid flake input package name")?;
            
        // Find the URL to update
        let language_fn = tree_sitter_nix::LANGUAGE;
        let language = unsafe { 
            tree_sitter::Language::from_raw(language_fn.into_raw()() as *const _) 
        };
        
        // Query for flake input URLs
        let query_str = r#"
        (binding
          (attrpath (identifier) @inputs_key)
          (attrset_expression
            (binding_set
              (binding
                (attrpath (identifier) @input_name . (identifier) @key)
                (string_expression (string_fragment) @url)
              ) @binding
            )
          )
        )
        "#;
        
        let query = Query::new(&language, query_str)?;
        let mut cursor = QueryCursor::new();
        let matches = cursor.matches(&query, tree.root_node(), content.as_bytes());
        
        let mut result = content.to_string();
        let mut offset_adjustment = 0i64;
        
        for match_ in matches {
            let mut is_target = false;
            let mut url_node = None;
            
            for capture in match_.captures {
                let capture_name = query.capture_names()[capture.index as usize];
                let text = &content[capture.node.byte_range()];
                
                match capture_name {
                    "inputs_key" if text == "inputs" => {},
                    "input_name" if text == input_name => is_target = true,
                    "key" if text == "url" => {},
                    "url" => url_node = Some(capture.node),
                    _ => {}
                }
            }
            
            if is_target {
                if let Some(node) = url_node {
                    let start = node.start_byte() as i64 + offset_adjustment;
                    let end = node.end_byte() as i64 + offset_adjustment;
                    
                    // Update the URL with the new version
                    let old_url = &result[start as usize..end as usize];
                    let new_url = self.update_flake_url(old_url, new_version)?;
                    
                    result.replace_range(start as usize..end as usize, &new_url);
                    offset_adjustment += new_url.len() as i64 - (end - start);
                }
            }
        }
        
        Ok(result)
    }
    
    fn update_flake_url(&self, old_url: &str, new_version: &str) -> Result<String> {
        // Handle different URL formats
        if old_url.starts_with("github:") {
            let parts: Vec<&str> = old_url.split('/').collect();
            if parts.len() >= 3 {
                // github:owner/repo/ref -> github:owner/repo/new_version
                Ok(format!("{}/{}/{}", parts[0], parts[1], new_version))
            } else if parts.len() == 2 {
                // github:owner/repo -> github:owner/repo/new_version
                Ok(format!("{}/{}", old_url, new_version))
            } else {
                Ok(old_url.to_string())
            }
        } else if old_url.contains("github.com") {
            // Handle various GitHub URL formats
            if let Some(ref_start) = old_url.find("?ref=") {
                // URL with ?ref= parameter
                Ok(format!("{}?ref={}", &old_url[..ref_start], new_version))
            } else if old_url.starts_with("https://github.com/") || old_url.starts_with("git+https://github.com/") {
                // Add ref parameter
                if old_url.contains('?') {
                    Ok(format!("{}&ref={}", old_url, new_version))
                } else {
                    Ok(format!("{}?ref={}", old_url, new_version))
                }
            } else {
                Ok(old_url.to_string())
            }
        } else {
            Ok(old_url.to_string())
        }
    }
    
    fn update_package_version(&self, content: &str, tree: &tree_sitter::Tree, _package: &Package, new_version: &str) -> Result<String> {
        let language_fn = tree_sitter_nix::LANGUAGE;
        let language = unsafe { 
            tree_sitter::Language::from_raw(language_fn.into_raw()() as *const _) 
        };
        
        // Query for version strings
        let query_str = r#"
        (binding
          (attrpath (identifier) @key)
          (string_expression (string_fragment) @value)
        ) @binding
        "#;
        
        let query = Query::new(&language, query_str)?;
        let mut cursor = QueryCursor::new();
        let matches = cursor.matches(&query, tree.root_node(), content.as_bytes());
        
        let mut result = content.to_string();
        let mut offset_adjustment = 0i64;
        
        for match_ in matches {
            let mut is_version = false;
            let mut value_node = None;
            
            for capture in match_.captures {
                let capture_name = query.capture_names()[capture.index as usize];
                let text = &content[capture.node.byte_range()];
                
                match capture_name {
                    "key" if text == "version" => is_version = true,
                    "value" => value_node = Some(capture.node),
                    _ => {}
                }
            }
            
            if is_version {
                if let Some(node) = value_node {
                    let start = node.start_byte() as i64 + offset_adjustment;
                    let end = node.end_byte() as i64 + offset_adjustment;
                    
                    result.replace_range(start as usize..end as usize, new_version);
                    offset_adjustment += new_version.len() as i64 - (end - start);
                }
            }
        }
        
        Ok(result)
    }
}

impl Updater for NixUpdater {
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
            path: "flake.nix".to_string(),
            file_type: FileType::Nix,
            name: name.to_string(),
            current_version: version.to_string(),
            sources: vec![SourceHint {
                source_type: SourceType::GitHub,
                identifier: "NixOS/nixpkgs".to_string(),
                url: None,
            }],
            update_strategy: UpdateStrategy::Stable,
            annotations: vec![],
            metadata: HashMap::new(),
        }
    }
    
    #[test]
    fn test_update_flake_github_shorthand() {
        let updater = NixUpdater::new();
        let content = r#"{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-23.11";
  };
}
"#;
        
        let package = create_test_package("flake-input-nixpkgs", "nixos-23.11");
        let result = updater.update_content(content, &package, "nixos-24.05").unwrap();
        
        assert!(result.contains("github:NixOS/nixpkgs/nixos-24.05"));
        assert!(!result.contains("nixos-23.11"));
    }
    
    #[test]
    fn test_update_flake_github_url_with_ref() {
        let updater = NixUpdater::new();
        let content = r#"{
  inputs = {
    myrepo.url = "https://github.com/user/repo?ref=v1.0.0";
  };
}
"#;
        
        let package = create_test_package("flake-input-myrepo", "v1.0.0");
        let result = updater.update_content(content, &package, "v2.0.0").unwrap();
        
        assert!(result.contains("https://github.com/user/repo?ref=v2.0.0"));
        assert!(!result.contains("v1.0.0"));
    }
    
    #[test]
    fn test_update_flake_github_url_without_ref() {
        let updater = NixUpdater::new();
        let content = r#"{
  inputs = {
    myrepo.url = "https://github.com/user/repo";
  };
}
"#;
        
        let package = create_test_package("flake-input-myrepo", "main");
        let result = updater.update_content(content, &package, "v1.0.0").unwrap();
        
        assert!(result.contains("https://github.com/user/repo?ref=v1.0.0"));
    }
    
    #[test]
    fn test_update_flake_git_plus_https() {
        let updater = NixUpdater::new();
        let content = r#"{
  inputs = {
    myrepo.url = "git+https://github.com/user/repo?ref=stable";
  };
}
"#;
        
        let package = create_test_package("flake-input-myrepo", "stable");
        let result = updater.update_content(content, &package, "v2.0").unwrap();
        
        assert!(result.contains("git+https://github.com/user/repo?ref=v2.0"));
        assert!(!result.contains("stable"));
    }
    
    #[test]
    fn test_update_package_version() {
        let updater = NixUpdater::new();
        let content = r#"{
  pname = "my-package";
  version = "1.0.0";
  src = fetchFromGitHub {
    owner = "user";
    repo = "repo";
    rev = "v1.0.0";
  };
}
"#;
        
        let package = create_test_package("package", "1.0.0");
        let result = updater.update_content(content, &package, "1.1.0").unwrap();
        
        assert!(result.contains(r#"version = "1.1.0""#));
        assert!(!result.contains(r#"version = "1.0.0""#));
        // Note: This doesn't update the rev field - that would need a more sophisticated approach
    }
    
    #[test]
    fn test_update_multiple_flake_inputs() {
        let updater = NixUpdater::new();
        let content = r#"{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-23.11";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };
}
"#;
        
        let package = create_test_package("flake-input-nixpkgs", "nixos-23.11");
        let result = updater.update_content(content, &package, "nixos-24.05").unwrap();
        
        assert!(result.contains("github:NixOS/nixpkgs/nixos-24.05"));
        // Other inputs should remain unchanged
        assert!(result.contains("github:numtide/flake-utils"));
        assert!(result.contains("github:oxalica/rust-overlay"));
    }
    
    #[test]
    fn test_preserve_formatting() {
        let updater = NixUpdater::new();
        let content = r#"{
  inputs = {
    # Main nixpkgs input
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-23.11";
    
    # Utility functions
    flake-utils.url = "github:numtide/flake-utils";
  };
}
"#;
        
        let package = create_test_package("flake-input-nixpkgs", "nixos-23.11");
        let result = updater.update_content(content, &package, "nixos-24.05").unwrap();
        
        // Comments should be preserved
        assert!(result.contains("# Main nixpkgs input"));
        assert!(result.contains("# Utility functions"));
    }
    
    #[test]
    fn test_no_update_when_input_not_found() {
        let updater = NixUpdater::new();
        let content = r#"{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-23.11";
  };
}
"#;
        
        let package = create_test_package("flake-input-notfound", "v1.0.0");
        let result = updater.update_content(content, &package, "v2.0.0").unwrap();
        
        // Content should remain unchanged
        assert_eq!(result, content);
    }
    
    #[test]
    fn test_update_flake_url_function() {
        let updater = NixUpdater::new();
        
        // Test github: shorthand
        assert_eq!(
            updater.update_flake_url("github:owner/repo/old-ref", "new-ref").unwrap(),
            "github:owner/repo/new-ref"
        );
        
        // Test github: without ref
        assert_eq!(
            updater.update_flake_url("github:owner/repo", "new-ref").unwrap(),
            "github:owner/repo/new-ref"
        );
        
        // Test https URL with ref
        assert_eq!(
            updater.update_flake_url("https://github.com/owner/repo?ref=old", "new").unwrap(),
            "https://github.com/owner/repo?ref=new"
        );
        
        // Test https URL without ref
        assert_eq!(
            updater.update_flake_url("https://github.com/owner/repo", "new").unwrap(),
            "https://github.com/owner/repo?ref=new"
        );
        
        // Test non-GitHub URL (should remain unchanged)
        assert_eq!(
            updater.update_flake_url("https://example.com/repo", "new").unwrap(),
            "https://example.com/repo"
        );
    }
}