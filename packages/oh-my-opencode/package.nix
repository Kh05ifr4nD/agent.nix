{
  lib,
  stdenv,
  fetchzip,
  makeWrapper,
  bun,
  versionCheckHook,
  versionCheckHomeHook,
}:

let
  versionData = builtins.fromJSON (builtins.readFile ./hashes.json);
  inherit (versionData) version hash;
in
stdenv.mkDerivation {
  pname = "oh-my-opencode";
  inherit version;

  src = fetchzip {
    url = "https://registry.npmjs.org/oh-my-opencode/-/oh-my-opencode-${version}.tgz";
    inherit hash;
  };

  nativeBuildInputs = [ makeWrapper ];

  dontBuild = true;

  installPhase = ''
    runHook preInstall

    mkdir -p $out/lib/$pname
    cp -r $src/* $out/lib/$pname/
    chmod -R u+w $out/lib/$pname

    # The bundled CLI embeds an outdated version string; patch it so `--version`
    # matches the derivation version (and works with versionCheckHook).
    sed -i '0,/^    name: "oh-my-opencode",$/ { /^    name: "oh-my-opencode",$/ { n; s/^    version: "[^"]*"/    version: "'"$version"'"/; } }' \
      $out/lib/$pname/dist/cli/index.js

    mkdir -p $out/bin
    makeWrapper ${bun}/bin/bun $out/bin/oh-my-opencode \
      --add-flags "$out/lib/$pname/dist/cli/index.js"

    runHook postInstall
  '';

  doInstallCheck = true;
  nativeInstallCheckInputs = [
    versionCheckHook
    versionCheckHomeHook
  ];
  versionCheckProgramArg = [ "--version" ];

  meta = with lib; {
    description = "OpenCode plugin - custom agents (oracle, librarian) and enhanced features";
    homepage = "https://github.com/code-yeongyu/oh-my-opencode";
    license = licenses.unfree;
    sourceProvenance = with sourceTypes; [ binaryNativeCode ];
    maintainers = with maintainers; [ ];
    mainProgram = "oh-my-opencode";
    platforms = [ "x86_64-linux" ];
  };
}
