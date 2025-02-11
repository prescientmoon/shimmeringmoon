{
  lib,

  pkg-config,
  makeWrapper,

  freetype,
  fontconfig,
  openssl,
  sqlite,
  makeRustPlatform,
  rust-toolchain,

  shimmering-fonts,
  cc-data,
  private-config,
}:
let
  src = lib.cleanSource ../.;
  rustPlatform = makeRustPlatform {
    cargo = rust-toolchain;
    rustc = rust-toolchain;
  };
in
rustPlatform.buildRustPackage {
  inherit src;
  pname = "shimmeringmoon";
  version = "unstable-2025-02-11";

  SHIMMERING_FONTS_DIR = shimmering-fonts;
  SHIMMERING_CC_DIR = cc-data;
  SHIMMERING_PRIVATE_CONFIG_DIR = private-config;

  nativeBuildInputs = [
    pkg-config
    rust-toolchain
    makeWrapper
  ];

  preBuild = ''
    export SHIMMERING_SOURCE_DIR="$src"
  '';

  # TODO: is this supposed to be here???
  # LD_LIBRARY_PATH = lib.makeLibraryPath buildInputs;
  buildInputs = [
    freetype
    fontconfig
    sqlite
    openssl
    src # Idk if putting this here is correct, but it is required at runtime...
  ];

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
}
