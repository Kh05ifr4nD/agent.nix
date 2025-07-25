use anyhow::{Context, Result};
use std::path::Path;

pub mod nix_updater;
pub mod cargo_updater;
pub mod npm_updater;
pub mod go_updater;

use crate::types::{FileType, Package};

pub trait Updater {
    /// Update a specific package in a file
    fn update_package(&self, file_path: &Path, package: &Package, new_version: &str) -> Result<String>;
}

pub struct UpdaterRegistry {
    nix_updater: nix_updater::NixUpdater,
    cargo_updater: cargo_updater::CargoUpdater,
    npm_updater: npm_updater::NpmUpdater,
    go_updater: go_updater::GoUpdater,
}

impl UpdaterRegistry {
    pub fn new() -> Self {
        Self {
            nix_updater: nix_updater::NixUpdater::new(),
            cargo_updater: cargo_updater::CargoUpdater::new(),
            npm_updater: npm_updater::NpmUpdater::new(),
            go_updater: go_updater::GoUpdater::new(),
        }
    }
    
    pub fn get_updater(&self, file_type: FileType) -> Option<&dyn Updater> {
        match file_type {
            FileType::Nix => Some(&self.nix_updater),
            FileType::CargoToml => Some(&self.cargo_updater),
            FileType::PackageJson => Some(&self.npm_updater),
            FileType::GoMod => Some(&self.go_updater),
            _ => None,
        }
    }
    
    pub fn update_file(&self, package: &Package, new_version: &str) -> Result<()> {
        let path = Path::new(&package.path);
        let updater = self.get_updater(package.file_type)
            .context("No updater available for this file type")?;
            
        let updated_content = updater.update_package(path, package, new_version)?;
        
        // Write the updated content back to the file
        std::fs::write(path, updated_content)
            .with_context(|| format!("Failed to write updated content to {:?}", path))?;
            
        Ok(())
    }
}