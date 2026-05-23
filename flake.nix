{
  description = "Devshell for Flint";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    fenix.url = "github:nix-community/fenix";
    fenix.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      fenix,
      ...
    }:
    let
      rev = self.shortRev or self.dirtyShortRev or "dirty";
    in
    {
      overlays.default = final: _: {
        flint = final.callPackage ./package.nix { inherit rev; };
      };
    }
    // flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs { inherit system; };
        fenixPkgs = fenix.packages.${system};

        rustToolchain = fenixPkgs.combine [
          fenixPkgs.stable.rustc
          fenixPkgs.stable.cargo
          fenixPkgs.stable.clippy
          fenixPkgs.default.rustfmt
          fenixPkgs.stable.rust-src
          fenixPkgs.stable.rust-std
        ];

        flint = pkgs.callPackage ./package.nix { inherit rev; };

        devShell = pkgs.mkShell {
          RUSTC_WRAPPER = "${pkgs.sccache}/bin/sccache";

          buildInputs = [
            rustToolchain
            fenixPkgs.stable.rust-analyzer
            pkgs.lefthook
            pkgs.cargo-binstall
            pkgs.sccache
            pkgs.cargo-nextest
            pkgs.just
            flint
          ];

          shellHook = ''
            lefthook install
            just --list
          '';
        };
      in
      {
        devShells.default = devShell;

        packages = {
          inherit flint;
          default = flint;
        };
      }
    );
}
