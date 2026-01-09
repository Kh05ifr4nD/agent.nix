{
  bash,
  coreutils,
  deno,
  fetchFromGitHub,
  lib,
  stdenv,
}:

stdenv.mkDerivation (finalAttrs: {
  pname = "sorapec";
  version = "0.17.2";

  src = fetchFromGitHub {
    owner = "Kh05ifr4nD";
    repo = "sorapec";
    rev = "1f1046e1684cbea3785737dc768445d7fa94d822";
    hash = "sha256-aeZTPE3TCriNQDngpkdWJEANgqTfcTOXathumKeRmIE=";
  };

  dontBuild = true;

  installPhase = ''
    runHook preInstall

    mkdir -p "$out/lib/sorapec"

    cp -R "$src/src" "$out/lib/sorapec/"
    cp -R "$src/template" "$out/lib/sorapec/"

    cp -v "$src/deno.json" "$out/lib/sorapec/"
    cp -v "$src/deno.lock" "$out/lib/sorapec/"
    cp -v "$src/package.json" "$out/lib/sorapec/"

    mkdir -p "$out/bin"

    cat > "$out/bin/sorapec" <<'EOF'
    #!@bash@
    set -euo pipefail

    cache_root="''${XDG_CACHE_HOME:-$HOME/.cache}"
    cache_dir="$cache_root/sorapec/@version@-$(basename "@out@")"
    installed_marker="$cache_dir/.installed"

    if [ ! -f "$installed_marker" ]; then
      @mkdir@ -p "$cache_dir"
      @cp@ -R "@out@/lib/sorapec/." "$cache_dir/"
      @chmod@ -R u+w "$cache_dir"
      @touch@ "$installed_marker"
    fi

    export DENO_DIR="''${DENO_DIR:-$cache_root/deno}"

    exec "@deno@" run -A --no-prompt --config "$cache_dir/deno.json" --sloppy-imports "$cache_dir/src/cli/index.ts" "$@"
    EOF

    substituteInPlace "$out/bin/sorapec" \
      --replace-fail "@bash@" "${bash}/bin/bash" \
      --replace-fail "@deno@" "${deno}/bin/deno" \
      --replace-fail "@mkdir@" "${coreutils}/bin/mkdir" \
      --replace-fail "@cp@" "${coreutils}/bin/cp" \
      --replace-fail "@chmod@" "${coreutils}/bin/chmod" \
      --replace-fail "@touch@" "${coreutils}/bin/touch" \
      --replace-fail "@out@" "$out" \
      --replace-fail "@version@" "${finalAttrs.version}"

    chmod +x "$out/bin/sorapec"

    runHook postInstall
  '';

  meta = {
    description = "AI-native system for spec-driven development";
    homepage = "https://github.com/Fission-AI/Sorapec";
    license = lib.licenses.mit;
    mainProgram = "sorapec";
    platforms = lib.platforms.unix;
  };
})
