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
        # pkgs = inputs.nixpkgs.legacyPackages.${system};
        pkgs = inputs.nixpkgs.legacyPackages.${system}.extend inputs.fenix.overlays.default;
        # pkgs = inputs.nixpkgs.legacyPackages.${system}.extend (import inputs.rust-overlay);
        # pkgs = import inputs.nixpkgs {
        #   inherit system;
        #   overlays = [ (import inputs.rust-overlay) ];
        # };
        # toolchain = pkgs.rust-bin.selectLatestNightlyWith (toolchain: toolchain.default);
        # toolchain = pkgs.rust-bin.stable.latest.default;
        rust-toolchain = pkgs.fenix.complete.toolchain;
        spkgs = inputs.self.packages.${system};
        inherit (pkgs) lib;
      in
      {
        packages = {
          inherit rust-toolchain;

          kazesawa = pkgs.callPackage ./nix/kazesawa.nix { };
          exo = pkgs.callPackage ./nix/exo.nix { };
          geosans-light = pkgs.callPackage ./nix/geosans-light.nix { };

          shimmering-fonts = pkgs.callPackage ./nix/fonts.nix {
            # Pass custom-packaged fonts
            inherit (spkgs) exo kazesawa geosans-light;
          };

          default = spkgs.shimmeringmoon;
          shimmeringmoon = pkgs.callPackage ./nix/shimmeringmoon.nix {
            inherit (spkgs) shimmering-fonts rust-toolchain;
          };
        };

        #  {{{ Devshell
        devShell = pkgs.mkShell rec {
          nativeBuildInputs = with pkgs; [
            # pkgs.cargo
            # pkgs.rustc
            # pkgs.clippy
            # pkgs.rust-analyzer
            # pkgs.rustfmt
            spkgs.rust-toolchain

            pkgs.ruff
            pkgs.imagemagick
            pkgs.pkg-config
          ];

          buildInputs = with pkgs; [
            freetype
            fontconfig
            leptonica
            tesseract
            sqlite
          ];

          LD_LIBRARY_PATH = lib.makeLibraryPath buildInputs;
          SHIMMERING_FONTS_DIR = spkgs.shimmering-fonts;
        };
        #  }}}
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
