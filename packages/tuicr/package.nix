{
  lib,
  fetchFromGitHub,
  rustPlatform,
  pkg-config,
  libgit2,
  git,
  util-linux,
  flake,
}:
rustPlatform.buildRustPackage (finalAttrs: {
  pname = "tuicr";
  version = "0.1.2";

  src = fetchFromGitHub {
    owner = "agavra";
    repo = "tuicr";
    tag = "v${finalAttrs.version}";
    hash = "sha256-Aggmmn8VfdOlDL2rqt88j2uUfN3W00pt4ks471MQHMU=";
  };

  cargoHash = "sha256-SS5gI5yq2O6jAMsnK9sw4VtJ1HkjdxG9bAaR0EzJL1s=";

  nativeBuildInputs = [
    pkg-config
  ];

  buildInputs = [
    libgit2
  ];

  doCheck = false;

  doInstallCheck = true;
  nativeInstallCheckInputs = [
    git
    util-linux
  ];
  installCheckPhase = ''
    runHook preInstallCheck

    # tuicr is a TUI program; run it inside a pseudo-TTY and quit immediately.
    tmpdir="$(mktemp -d)"
    trap 'rm -rf "$tmpdir"' EXIT
    cd "$tmpdir"

    git init --quiet
    git config user.email "test@test.com"
    git config user.name "Test"

    echo "initial content" > test.txt
    git add test.txt
    git commit -m initial --quiet

    echo "modified content" > test.txt

    printf 'q' | timeout 10 script -q -c "$out/bin/tuicr" /dev/null >/dev/null

    runHook postInstallCheck
  '';

  passthru.category = "Code Review";

  meta = {
    description = "Review AI-generated diffs like a GitHub pull request, right from your terminal";
    homepage = "https://github.com/agavra/tuicr";
    changelog = "https://github.com/agavra/tuicr/releases/tag/v${finalAttrs.version}";
    license = lib.licenses.mit;
    mainProgram = "tuicr";
    sourceProvenance = with lib.sourceTypes; [ fromSource ];
    platforms = lib.platforms.unix;
    maintainers = with flake.lib.maintainers; [ ypares ];
  };
})
