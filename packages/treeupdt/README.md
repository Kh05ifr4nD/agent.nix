# treeupdt - Tree Update Tool

Keep your dependency tree fresh! A Rust-based tool for scanning and updating dependencies across multiple package ecosystems.

## Features

- **Multi-language support**: Scan and update dependencies in Nix flakes, npm packages, Cargo projects, and Go modules
- **Smart version discovery**: Automatically fetch latest versions from GitHub, npm, crates.io, and other sources
- **Inline annotations**: Control update behavior with comments directly in your source files
- **Flexible filtering**: Filter packages by type, name pattern, source, or update strategy
- **Multiple output formats**: Text, JSON, YAML, or simple paths for scripting
- **Cache support**: Speed up repeated checks with built-in caching

## Installation

```bash
# Using Nix
nix run github:numtide/nix-ai-tools#treeupdt

# Build from source
cd packages/treeupdt
cargo build --release
./target/release/treeupdt
```

## Quick Start

```bash
# Scan current directory for all updatable packages
treeupdt scan .

# Scan a specific file
treeupdt scan Cargo.toml

# Check for available updates
treeupdt check

# Check with filters
treeupdt check --file-type cargo        # Only Cargo packages
treeupdt check --name-pattern "^serde"  # Packages starting with 'serde'
treeupdt check --source-type github     # Only GitHub sources

# Output formats
treeupdt scan . --output json          # JSON output
treeupdt check --output yaml           # YAML output
treeupdt check --output paths          # Just paths for scripting
```

## Inline Annotations

Control update behavior with inline comments:

```toml
# Cargo.toml
[dependencies]
serde = "1.0"  # treeupdt: ignore
tokio = "1.0"  # treeupdt: pin-version
reqwest = "0.11"  # treeupdt: update-strategy=conservative
```

```nix
# flake.nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-23.11";  # treeupdt: update-strategy=stable
    rust-overlay.url = "github:oxalica/rust-overlay";   # treeupdt: ignore
  };
}
```

## Annotation Directives

- `ignore` - Skip this dependency entirely
- `pin-version` - Never update this dependency
- `update-strategy` - Set update strategy: `stable`, `conservative`, `latest`, or `aggressive`
- `ignore-versions` - Regex pattern of versions to ignore (e.g., `ignore-versions=".*-rc.*"`)

## Configuration

Create a `.treeupdt.toml` file:

```
file:element@property

Examples:
  flake.nix:inputs.nixpkgs              # Update nixpkgs input
  packages/*/package.nix:version        # Update all package versions
  Cargo.toml:dependencies.serde         # Update specific dependency
  package.json:devDependencies.*        # Update all dev dependencies
```

## Annotations

Add hints in your source code to guide updates:

```nix
# package.nix
{
  version = "1.0.0"; # treeupdt: source=github:owner/repo strategy=conservative
  npmDepsHash = "sha256-..."; # treeupdt: auto-update
}
```

## License

MIT
