{
  description = "A Community Driven Application CLI Quick Script Tool";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    naersk.url = "github:nix-community/naersk";
  };

  outputs = { self, nixpkgs, flake-utils, naersk }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        naersk' = pkgs.callPackage naersk {};
      in
        {
          packages.default = naersk'.buildPackage {
            pname = "qst";
            src = ./.;
          };

          devShells.default = pkgs.mkShell {
            nativeBuildInputs = with pkgs; [
              rustc
              cargo
              rust-analyzer
              rustfmt
              clippy
              pkg-config
            ];
          };
        }
    );
}