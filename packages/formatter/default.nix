{
  flake,
  inputs,
  pkgs,
  ...
}:
# treefmt with config
let
  formatter = inputs.treefmt-nix.lib.mkWrapper pkgs {
    _file = __curPos.file;
    imports = [ ./treefmt.nix ];
  };

  check =
    pkgs.runCommand "format-check"
      {
        nativeBuildInputs = [
          formatter
          pkgs.git
        ];
      }
      ''
        export HOME=$NIX_BUILD_TOP/home

        # keep timestamps so that treefmt is able to detect mtime changes
        cp --preserve=mode,timestamps -r ${flake} source
        cd source
        chmod -R u+w .
        git init --quiet
        git add .
        treefmt --no-cache
        if ! git diff --exit-code; then
          echo "-------------------------------"
          echo "aborting due to above changes ^"
          exit 1
        fi
        touch $out
      '';

  denoCheck =
    pkgs.runCommand "deno-check"
      {
        nativeBuildInputs = [
          pkgs.deno
        ];
      }
      ''
        export HOME=$NIX_BUILD_TOP/home
        export DENO_DIR=$NIX_BUILD_TOP/deno-dir

        cp -r ${flake} source
        cd source
        chmod -R u+w .

        deno task lint
        deno task typecheck

        touch $out
      '';
in
formatter
// {
  passthru = formatter.passthru // {
    hideFromDocs = true;
    tests = {
      check = check;
      denoCheck = denoCheck;
    };
  };
}
