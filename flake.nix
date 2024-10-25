{
  description = "stellar-cli";

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
        stellardev = {
          name = "stellar-cli";
          src = ./.;
          nativeBuildInputs = pkgs.lib.optionals (pkgs.stdenv.isDarwin) [
            pkgs.darwin.apple_sdk.frameworks.SystemConfiguration
          ];
          buildInputs = with pkgs; [
            openssl
            pkg-config
            jq
            (rust-bin.stable.latest.default.override {
              extensions = [ "rust-src" ];
              targets = [ "wasm32-unknown-unknown" ];
            })
          ] ++ lib.optionals (stdenv.isLinux) [libudev-zero];
        };
        stellarcli = stellardev // {
          GIT_REVISION = "${self.rev or self.dirtyRev or "unknown"}";
          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          cargoLock.outputHashes = {
              # The only git+https dependency in Cargo.lock
              "stellar-rpc-client-21.4.0" = "sha256-gIxoWJY0MaV/cOmFdyclvjioCBkAhCAKFOvWc0q42/o=";
          };

          # As of writing 'cargo test' fails
          doCheck = false;
        };
        rustPlatformMod = pkgs.makeRustPlatform {
          cargo = pkgs.rust-bin.stable.latest.default;
          rustc = pkgs.rust-bin.stable.latest.default;
        };
      in
      with pkgs;
      {
        devShells.default = mkShell stellardev;
        packages.default = rustPlatformMod.buildRustPackage stellarcli;
        # {
        #   nativeBuildInputs = lib.optionals (stdenv.isDarwin) [
        #     pkgs.darwin.apple_sdk.frameworks.SystemConfiguration
        #   ];
        #   buildInputs = [
        #     openssl
        #     pkg-config
        #     jq
        #     (rust-bin.stable.latest.default.override {
        #       extensions = [ "rust-src" ];
        #       targets = [ "wasm32-unknown-unknown" ];
        #     })
        #   ] ++ lib.optionals (stdenv.isLinux) [libudev-zero];
        # };
      }
    );
}
