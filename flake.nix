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
          buildInputs = [
            openssl
            pkg-config
            libudev-zero
            jq
            (rust-bin.stable.latest.default.override {
              extensions = [ "rust-src" ];
              targets = [ "wasm32-unknown-unknown" ];
            })
          ];
          shellHook =
          ''
            echo "Using `nix --version`"
            alias stellar="cargo run --bin stellar --"
            SOROBAN_SECRET_KEY=SBPXU3OP65M2DQPRM4YFZ5NME66NWWH5DHN44IBDYAN4TFKKY7CS4OHY stellar keys add dev_key
            export STELLAR_NETWORK=testnet
            export STELLAR_ACCOUNT=dev_key
            shell=$SHELL
            shell=`basename $SHELL`
            source <(stellar completion --shell $shell)
          '';
        };
      }
    );
}
