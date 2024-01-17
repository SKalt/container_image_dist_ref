{
  description = "A library for parsing and validating container image distribution references.";
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-23.11";
    flake-utils.url = "github:numtide/flake-utils"; # TODO: pin
    rust-overlay = {
      url = "github:oxalica/rust-overlay"; # TODO: pin;
      inputs.flake-utils.follows = "flake-utils";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, flake-utils, nixpkgs, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = (import nixpkgs) {
          inherit system overlays;
        };
        # Generate a user-friendly version number.
        version = builtins.substring 0 8 self.lastModifiedDate;
        rust_toolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        info = (builtins.fromTOML (builtins.readFile ./Cargo.toml));
        filters = import ./nix/filter_filesets.nix { inherit (pkgs) lib; };
      in
      {
        packages = {
          # For `nix build`:
          default = pkgs.rustPlatform.buildRustPackage {
            # https://nixos.org/manual/nixpkgs/stable/#compiling-rust-applications-with-cargo
            # TODO: figure out how to avoid the benchmarks being run
            inherit version;
            pname = info.package.name;
            src = with pkgs.lib.fileset; toSource {
              root = ./. ;
              fileset = unions [ (filters.rust ./.) (./tests) (./src/domain/valid_ipv6.tsv) ];
              # ^ see https://johns.codes/blog/efficient-nix-derivations-with-file-sets
              # see https://github.com/JRMurr/roc2nix/blob/main/lib/languageFilters.nix
            };
            cargoLock = {
              lockFile = ./Cargo.lock;
            };
            nativeBuildInputs = [ rust_toolchain ];
          };
        };
        # For `nix develop`:
        devShell = pkgs.mkShell {
          # see https://github.com/NixOS/nixpkgs/issues/52447
          # see https://hoverbear.org/blog/rust-bindgen-in-nix/
          # see https://slightknack.dev/blog/nix-os-bindgen/
          # https://nixos.wiki/wiki/Rust#Installation_via_rustup
          nativeBuildInputs = [
            rust_toolchain
            pkgs.go
          ];
          buildInputs = with pkgs;
            [
              # rust tools
              rust-analyzer-unwrapped
              cargo-flamegraph

              # nix support
              nixpkgs-fmt
              nil

              # go tools
              gopls

              # other
              lychee
              shellcheck
              git
              bashInteractive
            ];

          # Environment variables
          RUST_SRC_PATH = "${rust_toolchain}/lib/rustlib/src/rust/library";
        };
      }
    );
}
