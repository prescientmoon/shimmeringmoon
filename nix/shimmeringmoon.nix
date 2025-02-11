{
  pkg-config,
  makeWrapper,

  freetype,
  fontconfig,
  openssl,
  sqlite,
  rustPlatform,

  shimmering-fonts,
  cc-data,
  private-config,
}:
rustPlatform.buildRustPackage rec {
  pname = "shimmeringmoon";
  version = "unstable-2025-02-11";
  src = ../.;

  SHIMMERING_FONT_DIR = shimmering-fonts;
  SHIMMERING_CC_DIR = cc-data;
  SHIMMERING_PRIVATE_CONFIG_DIR = private-config;

  nativeBuildInputs = [
    pkg-config
    makeWrapper
  ];

  buildInputs = [
    freetype
    fontconfig
    sqlite
    openssl
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

  postBuild = ''
    for file in $out/bin/*; do
      wrapProgram $file \
        --set SHIMMERING_CC_DIR "${cc-data}" \
        --set SHIMMERING_PRIVATE_CONFIG_DIR ${private-config}
    done
  '';

  meta = {
    description = "Arcaea score management toolchain";
    homepage = "https://git.moonythm.dev/prescientmoon/shimmeringmoon";
    mainProgram = "shimmering-cli";
    platforms = [ "x86_64-linux" ];
  };
}
