use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use colored::*;

mod cache;
mod config;
mod filter;
mod scanner;
mod sources;
mod types;
mod updater;

use crate::scanner::Registry;
use crate::filter::{Filter, FilterConfig};
use crate::config::Config;

#[derive(Parser)]
#[command(name = "treeupdt")]
#[command(about = "Keep your dependency tree fresh", version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormat {
    /// Human-readable text output
    Text,
    /// JSON output
    Json,
    /// YAML output
    Yaml,
    /// Simple paths for piping to update command
    Paths,
}

#[derive(Subcommand)]
enum Commands {
    /// Scan for updatable packages
    Scan {
        /// Path to scan
        #[arg(default_value = ".")]
        path: String,
        
        /// Enable verbose output
        #[arg(short, long)]
        verbose: bool,
        
        /// Output format
        #[arg(short = 'o', long, value_enum)]
        output: Option<OutputFormat>,
        
        /// Filter by file type (e.g., nix, cargo, npm)
        #[arg(short = 't', long)]
        file_type: Option<String>,
        
        /// Filter by package name pattern (regex)
        #[arg(short = 'n', long)]
        name_pattern: Option<String>,
        
        /// Filter by source type (github, npm, crates, git)
        #[arg(short = 's', long)]
        source_type: Option<String>,
        
        /// Filter by update strategy (stable, conservative, latest, aggressive)
        #[arg(short = 'u', long)]
        update_strategy: Option<String>,
    },
    
    /// Check for available updates
    Check {
        /// Enable verbose output
        #[arg(short, long)]
        verbose: bool,
        
        /// Disable cache
        #[arg(long)]
        no_cache: bool,
        
        /// Output format
        #[arg(short = 'o', long, value_enum)]
        output: Option<OutputFormat>,
        
        /// Filter by file type (e.g., nix, cargo, npm)
        #[arg(short = 't', long)]
        file_type: Option<String>,
        
        /// Filter by package name pattern (regex)
        #[arg(short = 'n', long)]
        name_pattern: Option<String>,
        
        /// Filter by source type (github, npm, crates, git)
        #[arg(short = 's', long)]
        source_type: Option<String>,
        
        /// Filter by update strategy (stable, conservative, latest, aggressive)
        #[arg(short = 'u', long)]
        update_strategy: Option<String>,
    },
    
    /// Update packages
    Update {
        /// Paths to update (e.g., flake.nix:inputs.nixpkgs)
        paths: Vec<String>,
        
        /// Enable verbose output
        #[arg(short, long)]
        verbose: bool,
    },
    
    /// Clear the cache
    ClearCache {
        /// Enable verbose output
        #[arg(short, long)]
        verbose: bool,
    },
    
    /// Generate example configuration file
    InitConfig {
        /// Path to write config file
        #[arg(default_value = ".treeupdt.toml")]
        path: String,
        
        /// Force overwrite existing file
        #[arg(short, long)]
        force: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Scan { path, verbose, output, file_type, name_pattern, source_type, update_strategy } => {
            let filter_config = FilterConfig {
                file_type,
                name_pattern,
                source_type,
                update_strategy,
            };
            run_scan(&path, verbose, output, filter_config)
        },
        Commands::Check { verbose, no_cache, output, file_type, name_pattern, source_type, update_strategy } => {
            let filter_config = FilterConfig {
                file_type,
                name_pattern,
                source_type,
                update_strategy,
            };
            run_check(verbose, no_cache, output, filter_config).await
        },
        Commands::Update { paths, verbose } => run_update(&paths, verbose).await,
        Commands::ClearCache { verbose } => run_clear_cache(verbose),
        Commands::InitConfig { path, force } => run_init_config(&path, force),
    }
}

fn run_scan(path: &str, _verbose: bool, output: Option<OutputFormat>, mut filter_config: FilterConfig) -> Result<()> {
    // Load configuration
    let config = Config::load_default().unwrap_or_default();
    
    // Merge config filters with CLI filters (CLI takes precedence)
    if filter_config.file_type.is_none() && config.global.filters.file_types.is_some() {
        filter_config.file_type = config.global.filters.file_types.as_ref().and_then(|v| v.first()).cloned();
    }
    if filter_config.name_pattern.is_none() && config.global.filters.name_patterns.is_some() {
        filter_config.name_pattern = config.global.filters.name_patterns.as_ref().and_then(|v| v.first()).cloned();
    }
    if filter_config.source_type.is_none() && config.global.filters.source_types.is_some() {
        filter_config.source_type = config.global.filters.source_types.as_ref().and_then(|v| v.first()).cloned();
    }
    if filter_config.update_strategy.is_none() && config.global.filters.update_strategies.is_some() {
        filter_config.update_strategy = config.global.filters.update_strategies.as_ref().and_then(|v| v.first()).cloned();
    }
    
    let registry = Registry::new();
    let mut packages = registry.scan(path)?;
    
    // Apply configuration-based filtering and modifications
    packages = packages.into_iter().filter_map(|mut pkg| {
        // Check if path is excluded
        if config.is_excluded(&pkg.path) {
            return None;
        }
        
        // Apply global default update strategy
        pkg.update_strategy = config.global.update_strategy;
        
        // Check file-level config
        if let Some(file_config) = config.get_file_config(&pkg.path) {
            if !file_config.enabled {
                return None;
            }
            
            // Apply file-level update strategy override
            if let Some(strategy) = file_config.update_strategy {
                pkg.update_strategy = strategy;
            }
            
            // Check package-level config within file
            if let Some(pkg_config) = file_config.packages.get(&pkg.name) {
                if !pkg_config.enabled {
                    return None;
                }
                
                // Apply pinned version
                if pkg_config.pin_version.is_some() {
                    return None; // Don't show pinned packages as updatable
                }
                
                // Apply package-specific update strategy
                if let Some(strategy) = pkg_config.update_strategy {
                    pkg.update_strategy = strategy;
                }
                
                // Apply preferred source
                if let Some(preferred) = &pkg_config.preferred_source {
                    // Move preferred source to front if it exists
                    if let Some(pos) = pkg.sources.iter().position(|s| &s.source_type == preferred) {
                        let source = pkg.sources.remove(pos);
                        pkg.sources.insert(0, source);
                    }
                }
            }
        }
        
        // Check global package config
        if let Some(pkg_config) = config.get_package_config(&pkg.name) {
            if !pkg_config.enabled {
                return None;
            }
            
            // Apply pinned version
            if pkg_config.pin_version.is_some() {
                return None; // Don't show pinned packages as updatable
            }
            
            // Apply package-specific update strategy
            if let Some(strategy) = pkg_config.update_strategy {
                pkg.update_strategy = strategy;
            }
            
            // Apply preferred source
            if let Some(preferred) = &pkg_config.preferred_source {
                // Move preferred source to front if it exists
                if let Some(pos) = pkg.sources.iter().position(|s| &s.source_type == preferred) {
                    let source = pkg.sources.remove(pos);
                    pkg.sources.insert(0, source);
                }
            }
        }
        
        // Apply annotations (highest priority)
        for annotation in &pkg.annotations {
            // Handle ignore directive
            if annotation.options.contains_key("ignore") {
                return None;
            }
            
            // Handle pin-version
            if annotation.options.contains_key("pin-version") {
                return None; // Pinned packages are not updatable
            }
            
            // Handle update-strategy
            if let Some(strategy_str) = annotation.options.get("update-strategy") {
                match strategy_str.as_str() {
                    "stable" => pkg.update_strategy = types::UpdateStrategy::Stable,
                    "conservative" => pkg.update_strategy = types::UpdateStrategy::Conservative,
                    "latest" => pkg.update_strategy = types::UpdateStrategy::Latest,
                    "aggressive" => pkg.update_strategy = types::UpdateStrategy::Aggressive,
                    _ => {}
                }
            }
        }
        
        Some(pkg)
    }).collect();
    
    // Apply CLI filters
    let filter = Filter::from_config(filter_config)?;
    packages = filter.apply(packages);
    
    match output {
        Some(OutputFormat::Json) => {
            let json = serde_json::to_string_pretty(&packages)?;
            println!("{}", json);
        }
        Some(OutputFormat::Yaml) => {
            let yaml = serde_yaml::to_string(&packages)?;
            println!("{}", yaml);
        }
        Some(OutputFormat::Paths) => {
            for pkg in &packages {
                println!("{}:{}", pkg.path, pkg.name);
            }
        }
        Some(OutputFormat::Text) | None => {
            println!("Scanning {} for updatable packages...", path);
            
            // Group packages by file
            let mut packages_by_file = std::collections::HashMap::new();
            for package in &packages {
                packages_by_file
                    .entry(&package.path)
                    .or_insert_with(Vec::new)
                    .push(package);
            }
            
            println!(
                "\nFound {} updatable items in {} files:\n",
                packages.len().to_string().bold(),
                packages_by_file.len().to_string().bold()
            );
            
            for (file_path, file_packages) in packages_by_file {
                println!("{}", file_path.cyan());
                for pkg in file_packages {
                    let mut line = format!(
                        "  └── {}: {}",
                        pkg.name.green(),
                        pkg.current_version.yellow()
                    );
                    
                    // Show source hints
                    if !pkg.sources.is_empty() {
                        let sources: Vec<String> = pkg.sources
                            .iter()
                            .map(|src| match &src.source_type {
                                types::SourceType::GitHub => format!("github:{}", src.identifier),
                                types::SourceType::Npm => format!("npm:{}", src.identifier),
                                types::SourceType::Git => format!("git:{}", src.identifier),
                                _ => src.identifier.clone(),
                            })
                            .collect();
                        line.push_str(&format!(" ({})", sources.join(", ")));
                    }
                    
                    // Show update strategy if not default
                    if pkg.update_strategy != types::UpdateStrategy::Stable {
                        let strategy = match pkg.update_strategy {
                            types::UpdateStrategy::Conservative => "conservative",
                            types::UpdateStrategy::Latest => "latest",
                            types::UpdateStrategy::Aggressive => "aggressive",
                            _ => "stable",
                        };
                        line.push_str(&format!(" [{}]", strategy.magenta()));
                    }
                    
                    println!("{}", line);
                }
            }
        }
    }
    
    Ok(())
}

async fn run_check(_verbose: bool, no_cache: bool, output: Option<OutputFormat>, mut filter_config: FilterConfig) -> Result<()> {
    // Load configuration
    let config = Config::load_default().unwrap_or_default();
    
    // Merge config filters with CLI filters (CLI takes precedence)
    if filter_config.file_type.is_none() && config.global.filters.file_types.is_some() {
        filter_config.file_type = config.global.filters.file_types.as_ref().and_then(|v| v.first()).cloned();
    }
    if filter_config.name_pattern.is_none() && config.global.filters.name_patterns.is_some() {
        filter_config.name_pattern = config.global.filters.name_patterns.as_ref().and_then(|v| v.first()).cloned();
    }
    if filter_config.source_type.is_none() && config.global.filters.source_types.is_some() {
        filter_config.source_type = config.global.filters.source_types.as_ref().and_then(|v| v.first()).cloned();
    }
    if filter_config.update_strategy.is_none() && config.global.filters.update_strategies.is_some() {
        filter_config.update_strategy = config.global.filters.update_strategies.as_ref().and_then(|v| v.first()).cloned();
    }
    
    // Scan for packages first
    let registry = Registry::new();
    let mut packages = registry.scan(".")?;
    
    // Apply configuration-based filtering and modifications
    packages = packages.into_iter().filter_map(|mut pkg| {
        // Check if path is excluded
        if config.is_excluded(&pkg.path) {
            return None;
        }
        
        // Apply global default update strategy
        pkg.update_strategy = config.global.update_strategy;
        
        // Check file-level config
        if let Some(file_config) = config.get_file_config(&pkg.path) {
            if !file_config.enabled {
                return None;
            }
            
            // Apply file-level update strategy override
            if let Some(strategy) = file_config.update_strategy {
                pkg.update_strategy = strategy;
            }
            
            // Check package-level config within file
            if let Some(pkg_config) = file_config.packages.get(&pkg.name) {
                if !pkg_config.enabled {
                    return None;
                }
                
                // Apply pinned version
                if pkg_config.pin_version.is_some() {
                    return None; // Don't show pinned packages as updatable
                }
                
                // Apply package-specific update strategy
                if let Some(strategy) = pkg_config.update_strategy {
                    pkg.update_strategy = strategy;
                }
                
                // Apply preferred source
                if let Some(preferred) = &pkg_config.preferred_source {
                    // Move preferred source to front if it exists
                    if let Some(pos) = pkg.sources.iter().position(|s| &s.source_type == preferred) {
                        let source = pkg.sources.remove(pos);
                        pkg.sources.insert(0, source);
                    }
                }
            }
        }
        
        // Check global package config
        if let Some(pkg_config) = config.get_package_config(&pkg.name) {
            if !pkg_config.enabled {
                return None;
            }
            
            // Apply pinned version
            if pkg_config.pin_version.is_some() {
                return None; // Don't show pinned packages as updatable
            }
            
            // Apply package-specific update strategy
            if let Some(strategy) = pkg_config.update_strategy {
                pkg.update_strategy = strategy;
            }
            
            // Apply preferred source
            if let Some(preferred) = &pkg_config.preferred_source {
                // Move preferred source to front if it exists
                if let Some(pos) = pkg.sources.iter().position(|s| &s.source_type == preferred) {
                    let source = pkg.sources.remove(pos);
                    pkg.sources.insert(0, source);
                }
            }
        }
        
        // Apply annotations (highest priority)
        for annotation in &pkg.annotations {
            // Handle ignore directive
            if annotation.options.contains_key("ignore") {
                return None;
            }
            
            // Handle pin-version
            if annotation.options.contains_key("pin-version") {
                return None; // Pinned packages are not updatable
            }
            
            // Handle update-strategy
            if let Some(strategy_str) = annotation.options.get("update-strategy") {
                match strategy_str.as_str() {
                    "stable" => pkg.update_strategy = types::UpdateStrategy::Stable,
                    "conservative" => pkg.update_strategy = types::UpdateStrategy::Conservative,
                    "latest" => pkg.update_strategy = types::UpdateStrategy::Latest,
                    "aggressive" => pkg.update_strategy = types::UpdateStrategy::Aggressive,
                    _ => {}
                }
            }
        }
        
        Some(pkg)
    }).collect();
    
    // Apply CLI filters
    let filter = Filter::from_config(filter_config)?;
    packages = filter.apply(packages);
    
    if packages.is_empty() {
        match output {
            Some(OutputFormat::Json) => println!("[]"),
            Some(OutputFormat::Yaml) => println!("[]"),
            _ => println!("No updatable packages found."),
        }
        return Ok(());
    }
    
    let use_cache = if no_cache { false } else { config.global.cache_enabled };
    let source_registry = sources::SourceRegistry::with_cache(use_cache);
    let mut updates = Vec::new();
    
    // Collect update information
    for package in &packages {
        for source_hint in &package.sources {
            if let Some(source) = source_registry.get_source(&source_hint.source_type) {
                match source.check_update(&source_hint.identifier, &package.current_version).await {
                    Ok(update_info) => {
                        if update_info.update_available {
                            // Check ignore_versions patterns
                            let mut should_ignore = false;
                            
                            // Check annotations first (highest priority)
                            for annotation in &package.annotations {
                                if let Some(ignore_pattern) = annotation.options.get("ignore-versions") {
                                    // Split by comma for multiple patterns
                                    for pattern in ignore_pattern.split(',') {
                                        let pattern = pattern.trim();
                                        if pattern.contains('*') {
                                            // Simple glob matching
                                            let regex_pattern = pattern.replace("*", ".*");
                                            if let Ok(re) = regex::Regex::new(&regex_pattern) {
                                                if re.is_match(&update_info.latest_version.version) {
                                                    should_ignore = true;
                                                    break;
                                                }
                                            }
                                        } else if pattern == &update_info.latest_version.version {
                                            should_ignore = true;
                                            break;
                                        }
                                    }
                                }
                            }
                            
                            // Check file-level package config
                            if !should_ignore {
                                if let Some(file_config) = config.get_file_config(&package.path) {
                                if let Some(pkg_config) = file_config.packages.get(&package.name) {
                                    for pattern in &pkg_config.ignore_versions {
                                        if pattern.contains('*') {
                                            // Simple glob matching
                                            let regex_pattern = pattern.replace("*", ".*");
                                            if let Ok(re) = regex::Regex::new(&regex_pattern) {
                                                if re.is_match(&update_info.latest_version.version) {
                                                    should_ignore = true;
                                                    break;
                                                }
                                            }
                                        } else if pattern == &update_info.latest_version.version {
                                            should_ignore = true;
                                            break;
                                        }
                                    }
                                }
                            }
                            }
                            
                            // Check global package config
                            if !should_ignore {
                                if let Some(pkg_config) = config.get_package_config(&package.name) {
                                    for pattern in &pkg_config.ignore_versions {
                                        if pattern.contains('*') {
                                            // Simple glob matching
                                            let regex_pattern = pattern.replace("*", ".*");
                                            if let Ok(re) = regex::Regex::new(&regex_pattern) {
                                                if re.is_match(&update_info.latest_version.version) {
                                                    should_ignore = true;
                                                    break;
                                                }
                                            }
                                        } else if pattern == &update_info.latest_version.version {
                                            should_ignore = true;
                                            break;
                                        }
                                    }
                                }
                            }
                            
                            if !should_ignore {
                                updates.push(serde_json::json!({
                                    "package": package.name,
                                    "path": package.path,
                                    "current_version": package.current_version,
                                    "latest_version": update_info.latest_version.version,
                                    "latest_stable_version": update_info.latest_stable_version.as_ref().map(|v| &v.version),
                                    "source_type": format!("{:?}", source_hint.source_type),
                                    "identifier": source_hint.identifier,
                                }));
                            }
                        }
                    }
                    Err(e) => {
                        if matches!(output, None | Some(OutputFormat::Text)) {
                            eprintln!("    Error checking {}: {}", package.name, e);
                        }
                    }
                }
            }
        }
    }
    
    match output {
        Some(OutputFormat::Json) => {
            println!("{}", serde_json::to_string_pretty(&updates)?);
        }
        Some(OutputFormat::Yaml) => {
            println!("{}", serde_yaml::to_string(&updates)?);
        }
        Some(OutputFormat::Text) | None => {
            println!("Checking for updates...");
            println!("\nChecking {} packages for updates...\n", packages.len());
            
            for update in &updates {
                let obj = update.as_object().unwrap();
                println!("  {}: {} -> {}", 
                    obj["package"].as_str().unwrap().cyan(), 
                    obj["current_version"].as_str().unwrap().yellow(), 
                    obj["latest_version"].as_str().unwrap().green()
                );
                if let Some(stable) = obj.get("latest_stable_version").and_then(|v| v.as_str()) {
                    if stable != obj["latest_version"].as_str().unwrap() {
                        println!("    (stable: {})", stable.green());
                    }
                }
            }
            
            if !updates.is_empty() {
                println!("\n{} updates available", updates.len().to_string().bold());
            } else {
                println!("\nAll packages are up to date!");
            }
        }
        Some(OutputFormat::Paths) => {
            // For check command with paths format, show only packages with updates
            for update in &updates {
                let obj = update.as_object().unwrap();
                println!("{}:{}", obj["path"].as_str().unwrap(), obj["package"].as_str().unwrap());
            }
        }
    }
    
    Ok(())
}

async fn run_update(paths: &[String], verbose: bool) -> Result<()> {
    if paths.is_empty() {
        println!("No paths specified. Use 'treeupdt scan --output paths' to see available update paths.");
        return Ok(());
    }
    
    let registry = Registry::new();
    let source_registry = sources::SourceRegistry::new();
    let updater_registry = updater::UpdaterRegistry::new();
    
    // First, scan for all packages
    let all_packages = registry.scan(".")?;
    
    for path_spec in paths {
        println!("Processing update: {}", path_spec.cyan());
        
        // Parse path specification (e.g., "flake.nix:flake-input-nixpkgs" or just "flake-input-nixpkgs")
        let (file_path, package_name) = if path_spec.contains(':') {
            let parts: Vec<&str> = path_spec.splitn(2, ':').collect();
            (Some(parts[0]), parts[1])
        } else {
            (None, path_spec.as_str())
        };
        
        // Find matching packages
        let matching_packages: Vec<&types::Package> = all_packages.iter()
            .filter(|pkg| {
                let name_matches = pkg.name == package_name;
                let path_matches = file_path.map_or(true, |fp| pkg.path.ends_with(fp));
                name_matches && path_matches
            })
            .collect();
            
        if matching_packages.is_empty() {
            eprintln!("  No package found matching: {}", path_spec);
            continue;
        }
        
        for package in matching_packages {
            println!("  Found: {} in {}", package.name.green(), package.path.cyan());
            
            // Check for updates
            let mut update_performed = false;
            for source_hint in &package.sources {
                if let Some(source) = source_registry.get_source(&source_hint.source_type) {
                    match source.check_update(&source_hint.identifier, &package.current_version).await {
                        Ok(update_info) => {
                            if update_info.update_available {
                                let new_version = &update_info.latest_version.version;
                                println!("    Updating {} -> {}", 
                                    package.current_version.yellow(), 
                                    new_version.green()
                                );
                                
                                // Perform the update
                                match updater_registry.update_file(package, new_version) {
                                    Ok(_) => {
                                        println!("    ✓ Updated successfully");
                                        update_performed = true;
                                        break; // Only use first successful source
                                    }
                                    Err(e) => {
                                        eprintln!("    ✗ Update failed: {}", e);
                                    }
                                }
                            } else {
                                println!("    Already up to date ({})", package.current_version.green());
                            }
                        }
                        Err(e) => {
                            if verbose {
                                eprintln!("    Error checking for updates: {}", e);
                            }
                        }
                    }
                }
            }
            
            if !update_performed && verbose {
                println!("    No updates available from any source");
            }
        }
    }
    
    Ok(())
}

fn run_clear_cache(verbose: bool) -> Result<()> {
    let cache = cache::Cache::new()?;
    cache.clear()?;
    
    if verbose {
        let cache_dir = dirs::cache_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine cache directory"))?
            .join("treeupdt");
        println!("Cleared cache at: {}", cache_dir.display());
    } else {
        println!("Cache cleared successfully");
    }
    
    Ok(())
}

fn run_init_config(path: &str, force: bool) -> Result<()> {
    let path = std::path::Path::new(path);
    
    // Check if file exists
    if path.exists() && !force {
        return Err(anyhow::anyhow!(
            "Configuration file already exists at {}. Use --force to overwrite.",
            path.display()
        ));
    }
    
    // Write example config
    std::fs::write(path, crate::config::EXAMPLE_CONFIG)?;
    
    println!("Created configuration file at: {}", path.display());
    println!("\nEdit this file to customize treeupdt behavior.");
    println!("See comments in the file for available options.");
    
    Ok(())
}