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
          name = "stellar";
          src = ./.;
          nativeBuildInputs = (pkgs.lib.optionals (pkgs.stdenv.isDarwin) [
            pkgs.darwin.apple_sdk.frameworks.SystemConfiguration
          ]) ++ (with pkgs; [
            pkg-config
          ]);
          buildInputs = with pkgs; [
            openssl
            jq
            dbus
            (rust-bin.stable.latest.default.override {
              extensions = [ "rust-src" ];
              targets = [ "wasm32v1-none" ];
            })
          ] ++ lib.optionals (stdenv.isLinux) [libudev-zero];
        };
        stellarcli = stellardev // {
          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          cargoLock.outputHashes = {
              # This is needed for any git+https dependency in Cargo.lock
              # "somepackage-1.2.3" = "sha256-somehash";
          };

          doCheck = false;

          GIT_REVISION = "${self.rev or self.dirtyRev or "unknown"}";
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
      }
    );
}
