{
  lib,
  rustPlatform,
  pkg-config,
  openssl,
}:

rustPlatform.buildRustPackage rec {
  pname = "treeupdt";
  version = "0.1.0";

  src = ./.;

  cargoHash = "sha256-DkLH1MXRPXMXekjAFpeAOodXSpL4zqD7LY3qW1eYxxY=";

  nativeBuildInputs = [ pkg-config ];
  buildInputs = [ openssl ];

  meta = with lib; {
    description = "Keep your dependency tree fresh with path-based updates";
    homepage = "https://github.com/numtide/nix-ai-tools/tree/main/packages/treeupdt";
    license = licenses.mit;
    maintainers = with maintainers; [ ];
    mainProgram = "treeupdt";
  };
}
