{
  unifont,
  exo,
  kazesawa,
  geosans-light,
  stdenvNoCC,
}:
stdenvNoCC.mkDerivation {
  name = "shimmering-fonts";
  dontUnpack = true;

  installPhase = ''
    mkdir -p $out
    cp "${unifont}/share/fonts/opentype/unifont.otf" $out
    cp "${exo}/share/fonts/truetype/Exo[wght].ttf" $out
    cp "${kazesawa}/share/fonts/truetype/Kazesawa-Regular.ttf" $out
    cp "${kazesawa}/share/fonts/truetype/Kazesawa-Bold.ttf" $out
    cp "${geosans-light}/share/fonts/truetype/GeosansLight.ttf" $out
  '';

  meta.description = "Collection of fonts required by `shimmeringmoon` at runtime";
}
