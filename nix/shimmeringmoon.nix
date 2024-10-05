{
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
  makeRustPlatform,

  rust-toolchain,
}:
(makeRustPlatform {
  cargo = rust-toolchain;
  rustc = rust-toolchain;
}).buildRustPackage
  rec {
    pname = "shimmeringmoon";
    version = "unstable-2024-09-06";
    src = lib.cleanSource ../.;

    nativeBuildInputs = [
      pkg-config
      makeWrapper
    ];

    # TODO: is this supposed to be here???
    # LD_LIBRARY_PATH = lib.makeLibraryPath buildInputs;
    buildInputs = [
      freetype
      fontconfig
      leptonica
      tesseract
      openssl
      sqlite
    ];

    cargoLock = {
      lockFile = ../Cargo.lock;
      outputHashes = {
        "hypertesseract-0.1.0" = "sha256-G0dos5yvvcfBKznAo1IIzLgXqRDxmyZwB93QQ6hVZSo=";
        "plotters-0.4.0" = "sha256-9wtd7lig1vQ2RJVaEHdicfPZy2AyuoNav8shPMZ1EuE=";
        "faer-0.19.4" = "sha256-VXMk2S3caMMs0N0PJa/m/7aPykYgeXVVn7GWPnG63nQ=";
      };
    };

    # Disable all tests
    doCheck = false;

    # Tell the binary where to find the fonts
    postInstall = ''
      wrapProgram $out/bin/shimmering-discord-bot \
        --set SHIMMERING_FONTS_DIR ${shimmering-fonts}
    '';

    meta = {
      description = "Arcaea score management toolchain";
      homepage = "https://github.com/prescientmoon/shimmeringmoon";
      mainProgram = "shimmering-cli";
      platforms = [ "x86_64-linux" ];
    };
  }
