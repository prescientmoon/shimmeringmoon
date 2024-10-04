{
  stdenvNoCC,
  fetchFromGitHub,
  lib,
}:
stdenvNoCC.mkDerivation {
  pname = "exo-1.0";
  version = "unstable-2021-08-26";
  src = fetchFromGitHub {
    owner = "NDISCOVER";
    repo = "Exo-1.0";
    rev = "3be4f55b626129f17a3b82677703e48c03dc2052";
    sha256 = "1l6k8q20anjcl3x7piwakgkdajkcjw70r6vfqxl8vycr0fra104d";
  };

  dontBuild = true;

  installPhase = ''
    runHook preInstall
    install -Dm644 $src/fonts/variable/*.ttf -t $out/share/fonts/truetype
    runHook postInstall
  '';

  meta = {
    description = "Exo 1.0 Font Family ";
    homepage = "https://fonts.google.com/specimen/Exo";
    platforms = with lib.platforms; all;
    license = lib.licenses.ofl;
  };
}
