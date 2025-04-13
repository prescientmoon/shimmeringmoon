{
  arcaea-ptt-data,
  lib,
  makeWrapper,
  python3,
  stdenvNoCC,
  symlinkJoin,
}:
let
  unpatched = stdenvNoCC.mkDerivation {
    name = "glass-server-db-updater";
    src = lib.fileset.toSource {
      root = ../scripts;
      fileset = lib.fileset.unions [ ../scripts/update-db-songs.py ];
    };

    buildPhase = ''
      runHook preBuild
      echo "#!${python3}/bin/python" > glass-server-db-updater
      cat $src/update-db-songs.py >> glass-server-db-updater
      chmod +x glass-server-db-updater
      runHook postBuild
    '';

    installPhase = ''
      runHook preInstall
      install -Dm755 glass-server-db-updater -t $out/bin/
      runHook postInstall
    '';

    meta = {
      description = "Arcaea private server database chart constant updater.";
      mainProgram = "glass-server-db-updater";
    };
  };
in
symlinkJoin {
  inherit (unpatched) name meta;
  paths = [ unpatched ];
  nativeBuildInputs = [ makeWrapper ];
  postBuild = ''
    wrapProgram $out/bin/glass-server-db-updater \
      --set SHIMMERING_CC_DIR "${arcaea-ptt-data}"
  '';
}
