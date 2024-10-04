{
  rustPlatform,
  lib,

  pkg-config,
  makeWrapper,

  freetype,
  fontconfig,
  leptonica,
  tesseract,
  openssl,
  sqlite,
  shimmering-fonts,
}:
rustPlatform.buildRustPackage {
  pname = "shimmeringmoon";
  version = "unstable-2024-09-06";

  nativeBuildInputs = [
    pkg-config
    makeWrapper
  ];

  buildInputs = [
    freetype
    fontconfig
    leptonica
    tesseract
    openssl
    sqlite
    shimmering-fonts
  ];

  # Tell the binary where to find the fonts
  # postBuild = ''
  #   wrapProgram $out/bin/shimmering-discord-bot \
  #     --set SHIMMERING_FONTS_DIR ${shimmering-fonts}
  # '';

  checkFlags = [
    # disable all tests
    "--skip"
  ];

  src = lib.cleanSource ../.;

  cargoLock = {
    lockFile = ../Cargo.lock;
    outputHashes = {
      "hypertesseract-0.1.0" = "sha256-G0dos5yvvcfBKznAo1IIzLgXqRDxmyZwB93QQ6hVZSo=";
      "plotters-0.4.0" = "sha256-9wtd7lig1vQ2RJVaEHdicfPZy2AyuoNav8shPMZ1EuE=";
    };
  };
}
