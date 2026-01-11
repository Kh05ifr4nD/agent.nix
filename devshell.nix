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
    pkgs.python3

    # Formatter
    perSystem.self.formatter
  ];

  shellHook = ''
    export PRJ_ROOT=$PWD
  '';
}
