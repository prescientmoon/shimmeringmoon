{
  lib,
  pkg-config,
  makeWrapper,
  symlinkJoin,

  freetype,
  fontconfig,
  openssl,
  sqlite,
  rustPlatform,

  shimmering-fonts,
  arcaea-ptt-data,
  shimmering-private-config,
  shimmeringdarkness,
}:
let
  # We bake the env vars into the binaries in a separate derivation,
  # such that changing cc data/the content bundle doesn't rebuild the bot.
  unpatched = rustPlatform.buildRustPackage {
    pname = "shimmeringmoon";
    version = "unstable-2025-02-11";
    src = lib.fileset.toSource {
      root = ../.;
      fileset = lib.fileset.unions [
        ../Cargo.lock
        ../Cargo.toml
        ../migrations
        ../src
      ];
    };

    SHIMMERING_FONT_DIR = shimmering-fonts;
    SHIMMERING_COMPTIME_PRIVATE_CONFIG_DIR = shimmeringdarkness;

    nativeBuildInputs = [ pkg-config ];

    buildInputs = [
      freetype
      fontconfig
      sqlite
      openssl
    ];

    useFetchCargoVendor = true;
    cargoLock = {
      lockFile = ../Cargo.lock;
      outputHashes = {
        "plotters-0.4.0" = "sha256-9wtd7lig1vQ2RJVaEHdicfPZy2AyuoNav8shPMZ1EuE=";
        "faer-0.19.4" = "sha256-VXMk2S3caMMs0N0PJa/m/7aPykYgeXVVn7GWPnG63nQ=";
        "poise-0.6.1" = "sha256-44pPe02JJ97GEpzAXdQmDq/9bb4KS9G7ZFVlBRC6EYs=";
      };
    };

    # Disable all tests
    doCheck = false;

    meta = {
      description = "Arcaea score management toolchain";
      homepage = "https://git.moonythm.dev/prescientmoon/shimmeringmoon";
      mainProgram = "shimmering-cli";
      platforms = [ "x86_64-linux" ];
    };
  };
in
symlinkJoin {
  inherit (unpatched) name meta;
  paths = [ unpatched ];
  nativeBuildInputs = [ makeWrapper ];
  postBuild = ''
    for file in $out/bin/*; do
      wrapProgram $file \
        --set SHIMMERING_CC_DIR "${arcaea-ptt-data}" \
        --set SHIMMERING_PRIVATE_CONFIG_DIR ${shimmering-private-config}
    done
  '';
}
