{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";

    shimmeringdarkness.url = "git+ssh://forgejo@ssh.git.moonythm.dev/prescientmoon/shimmeringdarkness.git";
    shimmeringdarkness.flake = false;
  };

  outputs =
    inputs:
    inputs.flake-utils.lib.eachSystem (with inputs.flake-utils.lib.system; [ x86_64-linux ]) (
      system:
      let
        pkgs = inputs.nixpkgs.legacyPackages.${system};
        spkgs = inputs.self.packages.${system};
        inherit (pkgs) lib;
      in
      {
        packages = {
          # {{{ Private config
          shimmeringdarkness = inputs.shimmeringdarkness.outPath;
          glass-bundler = pkgs.callPackage ./nix/glass-bundler.nix { };
          debundled-darkness = pkgs.callPackage ./nix/debundled-darkness.nix {
            inherit (spkgs) shimmeringdarkness glass-bundler;
          };

          private-config = pkgs.callPackage ./nix/private-config.nix {
            inherit (spkgs) shimmeringdarkness debundled-darkness;
          };
          # }}}
          # {{{ Fonts
          kazesawa = pkgs.callPackage ./nix/kazesawa.nix { };
          exo = pkgs.callPackage ./nix/exo.nix { };
          geosans-light = pkgs.callPackage ./nix/geosans-light.nix { };

          shimmering-fonts = pkgs.callPackage ./nix/fonts.nix {
            # Pass custom-packaged fonts
            inherit (spkgs) exo kazesawa geosans-light;
          };
          # }}}
          # {{{ Shimmeringmoon
          cc-data = pkgs.callPackage ./nix/cc-data.nix { };
          default = spkgs.shimmeringmoon;
          shimmeringmoon = pkgs.callPackage ./nix/shimmeringmoon.nix {
            inherit (spkgs)
              shimmering-fonts
              cc-data
              private-config
              ;
          };
          # }}}
        };

        #  {{{ Devshell
        devShell = pkgs.mkShell rec {
          nativeBuildInputs = [
            pkgs.rustc
            pkgs.cargo
            pkgs.rustfmt
            pkgs.clippy
            pkgs.rust-analyzer

            pkgs.ruff
            pkgs.imagemagick
            pkgs.pkg-config
          ];

          buildInputs = with pkgs; [
            python3
            freetype
            fontconfig
            sqlite
            openssl
          ];

          LD_LIBRARY_PATH = lib.makeLibraryPath buildInputs;
          SHIMMERING_FONT_DIR = spkgs.shimmering-fonts;
          SHIMMERING_CC_DIR = spkgs.cc-data;
          SHIMMERING_PRIVATE_CONFIG_DIR = spkgs.private-config;
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
