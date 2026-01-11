{ pkgs, perSystem }:
pkgs.mkShellNoCC {
  packages = [
    # Tools needed for update scripts
    pkgs.bash
    pkgs.coreutils
    pkgs.curl
    pkgs.gh
    pkgs.git
    pkgs.gnugrep
    pkgs.gnused
    pkgs.jq
    pkgs.nix-prefetch-scripts
    pkgs.nix-update
    pkgs.nodejs
    pkgs.deno

    # Agents
    perSystem.self.opencode
    perSystem.self."oh-my-opencode"

    # Formatter
    perSystem.self.formatter
  ];

  shellHook = ''
    export PRJ_ROOT="$PWD"
    export OPENCODE_CONFIG_DIR="$PRJ_ROOT/.opencode"
  '';
}
