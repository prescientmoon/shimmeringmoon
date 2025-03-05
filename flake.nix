{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";

    shimmeringdarkness.url = "git+ssh://forgejo@ssh.git.moonythm.dev/prescientmoon/shimmeringdarkness.git";
    shimmeringdarkness.flake = false;

    shimmeringvoid.url = "git+ssh://forgejo@ssh.git.moonythm.dev/prescientmoon/shimmeringvoid.git";
    shimmeringvoid.flake = false;
  };

  outputs =
    inputs:
    {
      overlays.default = (import ./nix/overlay.nix { inherit inputs; });
    }
    // inputs.flake-utils.lib.eachSystem (with inputs.flake-utils.lib.system; [ x86_64-linux ]) (
      system:
      let
        pkgs = import inputs.nixpkgs {
          inherit system;
          overlays = [ inputs.self.overlays.default ];
        };
      in
      {
        packages = {
          inherit (pkgs) shimmeringmoon private-config;
          default = pkgs.shimmeringmoon;
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

          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath buildInputs;
          SHIMMERING_FONT_DIR = pkgs.shimmering-fonts;
          SHIMMERING_CC_DIR = pkgs.arcaea-ptt-data;
          SHIMMERING_PRIVATE_CONFIG_DIR = pkgs.private-config;
          SHIMMERING_PRIVATE_COMPTIME_CONFIG_DIR = inputs.shimmeringdarkness;
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
