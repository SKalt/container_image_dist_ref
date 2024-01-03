{
  description = ""; # FIXME: add a description
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable"; # TODO: pin
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
          # For `nix build` & `nix run`:
          default = pkgs.rustPlatform.buildRustPackage {
            # https://nixos.org/manual/nixpkgs/stable/#compiling-rust-applications-with-cargo
            inherit version;
            pname = info.package.name;
            src = pkgs.lib.fileset.toSource {
              root = ./. ;
              fileset = filters.rust ./. ;
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
          nativeBuildInputs = [ rust_toolchain ];
          buildInputs = with pkgs;
            [
              # rust tools
              rust-analyzer-unwrapped
              cargo-bloat

              # nix support
              nixpkgs-fmt
              nil

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
