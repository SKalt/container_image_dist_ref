# adapted from https://github.com/JRMurr/roc2nix/blob/main/lib/languageFilters.nix
{ lib }:

# common file sets for different languages
# https://nix.dev/tutorials/file-sets
# https://nixos.org/manual/nixpkgs/unstable/#sec-functions-library-fileset
with lib.fileset;

let

  fileHasAnySuffix = fileSuffixes: file: (lib.lists.any (s: lib.hasSuffix s file.name) fileSuffixes);
  rust = basePath: (
    let
      mainFilter = fileFilter
        (fileHasAnySuffix [ ".rs" "Cargo.toml"])
        basePath;
    in
    unions [ mainFilter (basePath + "/Cargo.toml") (basePath + "/Cargo.lock") ]
  );
  go = basePath: (
    let
      mainFilter = fileFilter
        (fileHasAnySuffix [ ".go"])
        basePath;
    in
    unions [ mainFilter (basePath + "/go.mod") (basePath + "/go.sum") ]
  );
  tsv = basePath: (
    let
      mainFilter = fileFilter
        (fileHasAnySuffix [ ".tsv"])
        basePath;
    in
    unions [ mainFilter ]
  );
  in
{
  inherit rust go tsv;
}
