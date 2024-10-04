{
  stdenvNoCC,
  fetchzip,
  lib,
}:
stdenvNoCC.mkDerivation {
  name = "geosans-light";

  src = fetchzip {
    url = "https://dl.dafont.com/dl/?f=geo_sans_light";
    sha256 = "sha256-T4N+c8pNaak2cC9WQqj8iezqVs47COIrUJv5yvpEBH4=";
    extension = "zip"; # The build otherwise fails because of the query param
    stripRoot = false;
  };

  installPhase = ''
    runHook preInstall
    install -Dm644 $src/*.ttf -t $out/share/fonts/truetype
    runHook postInstall
  '';

  meta = {
    description = "Exo 1.0 Font Family ";
    homepage = "https://www.dafont.com/geo-sans-light.font";
    platforms = with lib.platforms; all;
  };
}
