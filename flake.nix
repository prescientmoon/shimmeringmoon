{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/release-24.05";
    flake-utils.url = "github:numtide/flake-utils";
    fenix.url = "github:nix-community/fenix";
    fenix.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs =
    { ... }@inputs:
    inputs.flake-utils.lib.eachSystem (with inputs.flake-utils.lib.system; [ x86_64-linux ]) (
      system:
      let
        pkgs = inputs.nixpkgs.legacyPackages.${system}.extend inputs.fenix.overlays.default;
        inherit (pkgs) lib;
      in
      {
        packages.shimmeringmoon = pkgs.rustPlatform.buildRustPackage {
          pname = "shimmeringmoon";
          version = "unstable-2024-09-06";

          src = lib.cleanSource ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
            outputHashes = {
              "hypertesseract-0.1.0" = "sha256-G0dos5yvvcfBKznAo1IIzLgXqRDxmyZwB93QQ6hVZSo=";
              "plotters-0.4.0" = "sha256-9wtd7lig1vQ2RJVaEHdicfPZy2AyuoNav8shPMZ1EuE=";
            };
          };
        };
        devShell = pkgs.mkShell rec {
          packages = with pkgs; [
            (fenix.complete.withComponents [
              "cargo"
              "clippy"
              "rust-src"
              "rustc"
              "rustfmt"
            ])
            rust-analyzer-nightly
            ruff
            imagemagick
            fontconfig
            freetype

            clang
            llvmPackages.clang
            pkg-config

            leptonica
            tesseract
            openssl
            sqlite
          ];

          LD_LIBRARY_PATH = lib.makeLibraryPath packages;

          # compilation of -sys packages requires manually setting LIBCLANG_PATH
          LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
        };
      }
    );

  # {{{ Caching and whatnot
  # TODO: persist trusted substituters file
  nixConfig = {
    extra-substituters = [ "https://nix-community.cachix.org" ];

    extra-trusted-public-keys = [
      "nix-community.cachix.org-1:mB9FSh9qf2dCimDSUo8Zy7bkq5CX+/rkCWyvRCYg3Fs="
    ];
  };
  # }}}
}
