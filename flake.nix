{
  description = "revm running inside cosmwasm";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

    crane.url = "github:ipetkov/crane";

    flake-utils.url = "github:numtide/flake-utils";

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, crane, flake-utils, rust-overlay, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };

        nightlyVersion = "2025-02-16";
        channel = "nightly-${nightlyVersion}";

        rustToolchain = pkgs.rust-bin.fromRustupToolchain {
          inherit channel;
          targets = [ "wasm32-unknown-unknown" ];
          profile = "minimal";
          components = [
            "rustc"
            "cargo"
            "rustfmt"
            "rust-std"
            "rust-docs"
            "rust-analyzer"
            "clippy"
            "miri"
            "rust-src"
            "llvm-tools-preview"
          ];
        };

        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

        cargoVendorDir = craneLib.vendorMultipleCargoDeps {
          cargoLockList =
            [
              ./Cargo.lock
              "${rustToolchain.passthru.availableComponents.rust-src}/lib/rustlib/src/rust/library/Cargo.lock"
            ];
        };

        cosmwasm-evm = craneLib.buildPackage {
          src = craneLib.cleanCargoSource ./.;
          strictDeps = true;

          preBuild=''
            echo "cargoVendorDir: ${cargoVendorDir}"
            echo "rustToolchain: ${rustToolchain}"

            find ${cargoVendorDir} -maxdepth 1 -xtype d | grep -v '^${cargoVendorDir}$' | sed -E 's@(.+)@ --remap-path-prefix=\1=/@g'

            export RUSTFLAGS="$RUSTFLAGS $(find ${cargoVendorDir} -maxdepth 1 -xtype d | grep -v '^${cargoVendorDir}$' | sed -E 's@(.+)@ --remap-path-prefix=\1=@g' | tr '\n' ' ') --remap-path-prefix=${rustToolchain}/lib/rustlib/src/rust/library/alloc/src/= --remap-path-prefix=${rustToolchain}/lib/rustlib/src/rust/library/std/src/= --remap-path-prefix=${rustToolchain}/lib/rustlib/src/rust/library/core/src/= -Zlocation-detail=none"

            echo "$RUSTFLAGS"
          '';

          cargoExtraArgs = "--lib --target wasm32-unknown-unknown -j1 -Z build-std=std,panic_abort -Z build-std-features=panic_immediate_abort";

          doCheck = false;

          inherit cargoVendorDir;

          installPhase = ''
            ${pkgs.binaryen}/bin/wasm-opt -Oz target/wasm32-unknown-unknown/release/cosmwasm_evm.wasm -o $out --strip --strip-producers --strip-eh --strip-dwarf
            # running with -Os on the output of -Oz produces a slightly smaller binary
            # about 1.5kb, or ~0.26% (lol)
            ${pkgs.binaryen}/bin/wasm-opt -Os $out -o $out
          '';
        };

        cosmwasm-evm-schema = pkgs.stdenv.mkDerivation {
          name = "coswasm-evm-schema";
          version = "0.0.0";
          src = craneLib.cleanCargoSource ./.;
          buildInputs = [(craneLib.buildPackage {
            src = craneLib.cleanCargoSource ./.;
            strictDeps = true;

            CARGO_PROFILE = "dev";

            doCheck = false;

            meta.mainProgram = "schema";
          })];
          buildPhase = ''
            schema
            mv ./schema $out
          '';
        };
      in
      {
        checks = {
          inherit cosmwasm-evm;
        };

        packages = {
          inherit cosmwasm-evm;
          inherit cosmwasm-evm-schema;
        };

        devShells.default = craneLib.devShell {
          checks = self.checks.${system};
          packages = [
            pkgs.binaryen
            pkgs.nodejs
            pkgs.rust-bin.stable.latest.default
          ];
        };
      });
}
