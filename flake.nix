{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    fenix.url = "github:nix-community/fenix";
    fenix.inputs.nixpkgs.follows = "nixpkgs";
    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs =
    inputs:
    inputs.flake-utils.lib.eachSystem (with inputs.flake-utils.lib.system; [ x86_64-linux ]) (
      system:
      let
        pkgs = inputs.nixpkgs.legacyPackages.${system};
        # pkgs = inputs.nixpkgs.legacyPackages.${system}.extend (import inputs.rust-overlay);
        # pkgs = import inputs.nixpkgs {
        #   inherit system;
        #   overlays = [ (import inputs.rust-overlay) ];
        # };
        # toolchain = pkgs.rust-bin.selectLatestNightlyWith (toolchain: toolchain.default);
        # toolchain = pkgs.rust-bin.stable.latest.default;
        toolchain = inputs.fenix.packages.${system}.complete.toolchain;
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
          nativeBuildInputs = with pkgs; [
            toolchain
            ruff
            imagemagick
            pkg-config

            # clang
            # llvmPackages.clang
          ];
          buildInputs = with pkgs; [
            freetype
            fontconfig
            leptonica
            tesseract
            # openssl
            sqlite
          ];

          LD_LIBRARY_PATH = lib.makeLibraryPath buildInputs;

          # compilation of -sys packages requires manually setting LIBCLANG_PATH
          # LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
        };
      }
    );

  # {{{ Caching and whatnot
  nixConfig = {
    extra-substituters = [ "https://nix-community.cachix.org" ];

    extra-trusted-public-keys = [
      "nix-community.cachix.org-1:mB9FSh9qf2dCimDSUo8Zy7bkq5CX+/rkCWyvRCYg3Fs="
    ];
  };
  # }}}
}
