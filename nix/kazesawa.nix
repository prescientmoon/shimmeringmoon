{
  stdenvNoCC,
  fetchzip,
  lib,
}:
stdenvNoCC.mkDerivation rec {
  pname = "kazesawa";
  version = "alpha-v1";
  src = fetchzip {
    url = "https://github.com/kazesawa/kazesawa/releases/download/${version}/kazesawa.zip";
    sha256 = "JM6QfpsoWFQh4jUODflLOwoGoRaq8UqFnaGElzkT/H4=";
    stripRoot = false;
  };

  installPhase = ''
    runHook preInstall
    install -Dm644 *.ttf -t $out/share/fonts/truetype
    runHook postInstall
  '';

  meta = {
    description = "Kazesawa Font: M+ with Source Sans Pro";
    homepage = "https://kazesawa.github.io/";
    platforms = with lib.platforms; all;
    license = lib.licenses.ofl;
  };
}
