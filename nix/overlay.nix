{ inputs }:
final: prev: {
  shimmeringvoid = inputs.shimmeringvoid.outPath;
  shimmeringdarkness = inputs.shimmeringdarkness.outPath;
  glass-charts = inputs.glass-charts.outPath;
  glass-maps = inputs.glass-maps.outPath;
  arcaea-ptt-data = inputs.arcaea-ptt-data.outPath;

  shimmeringextra = final.callPackage ./shimmeringextra.nix { };
  glass-bundler = final.callPackage ./glass-bundler.nix { };
  debundled-void = final.callPackage ./debundled-void.nix { };
  shimmering-private-config = final.callPackage ./private-config.nix { };

  kazesawa = final.callPackage ./kazesawa.nix { };
  exo = final.callPackage ./exo.nix { };
  geosans-light = final.callPackage ./geosans-light.nix { };
  shimmering-fonts = final.callPackage ./fonts.nix { };

  shimmeringmoon = final.callPackage ./shimmeringmoon.nix { };
  glass-server-db-updater = final.callPackage ./glass-server-db-updater.nix { };
  chart-dir-checker = final.callPackage ./chart-dir-checker.nix { };
}
