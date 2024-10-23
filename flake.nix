{
  description = "stellar-cli development shell";

  inputs = {
    nixpkgs.url      = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url  = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
      in
      with pkgs;
      {
        devShells.default = mkShell {
          nativeBuildInputs = lib.optionals (stdenv.isDarwin) [
            pkgs.darwin.apple_sdk.frameworks.SystemConfiguration
          ];
          buildInputs = [
            openssl
            pkg-config
            jq
            (rust-bin.stable.latest.default.override {
              extensions = [ "rust-src" ];
              targets = [ "wasm32-unknown-unknown" ];
            })
          ] ++ lib.optionals (stdenv.isLinux) [libudev-zero];
        };
      }
    );
}
