pub mod annotation_parser;
pub mod cargo_scanner;
pub mod go_scanner;
pub mod nix_ast_scanner;
pub mod npm_scanner;

use crate::types::{Package, Scanner};
use anyhow::Result;

pub use self::cargo_scanner::CargoScanner;
pub use self::go_scanner::GoModScanner;
pub use self::nix_ast_scanner::NixAstScanner;
pub use self::npm_scanner::NpmScanner;

pub struct Registry {
    scanners: Vec<Box<dyn Scanner>>,
}

impl Registry {
    pub fn new() -> Self {
        Self {
            scanners: vec![
                Box::new(NixAstScanner::new()),  // Use AST scanner with tree-sitter
                Box::new(GoModScanner::new()),
                Box::new(NpmScanner::new()),
                Box::new(CargoScanner::new()),
            ],
        }
    }
    
    pub fn scan(&self, root_path: &str) -> Result<Vec<Package>> {
        let mut all_packages = Vec::new();
        
        for scanner in &self.scanners {
            match scanner.scan(root_path) {
                Ok(packages) => all_packages.extend(packages),
                Err(e) => eprintln!("Scanner error: {}", e),
            }
        }
        
        Ok(all_packages)
    }
}