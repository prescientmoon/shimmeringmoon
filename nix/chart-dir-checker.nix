{
  lib,
  makeWrapper,
  python3,
  stdenvNoCC,
  symlinkJoin,
  shimmering-private-config,
}:
let
  unpatched = stdenvNoCC.mkDerivation {
    name = "chart-dir-checker";
    src = lib.fileset.toSource {
      root = ../scripts;
      fileset = lib.fileset.unions [ ../scripts/chart-dir-checker.py ];
    };

    buildPhase = ''
      runHook preBuild
      echo "#!${python3}/bin/python" > chart-dir-checker
      cat $src/chart-dir-checker.py >> chart-dir-checker
      chmod +x chart-dir-checker
      runHook postBuild
    '';

    installPhase = ''
      runHook preInstall
      install -Dm755 chart-dir-checker -t $out/bin/
      runHook postInstall
    '';

    meta = {
      description = "Arcaea private server chart dir validator.";
      mainProgram = "chart-dir-checker";
    };
  };
in
symlinkJoin {
  inherit (unpatched) name meta;
  paths = [ unpatched ];
  nativeBuildInputs = [ makeWrapper ];
  postBuild = ''
    wrapProgram $out/bin/chart-dir-checker \
      --set SHIMMERING_PRIVATE_CONFIG_DIR ${shimmering-private-config}
  '';
}
