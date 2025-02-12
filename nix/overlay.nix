{ inputs }:
final: prev: {
  shimmeringdarkness = inputs.shimmeringdarkness.outPath;
  glass-bundler = final.callPackage ./glass-bundler.nix { };
  debundled-darkness = final.callPackage ./debundled-darkness.nix { };
  private-config = final.callPackage ./private-config.nix { };

  kazesawa = final.callPackage ./kazesawa.nix { };
  exo = final.callPackage ./exo.nix { };
  geosans-light = final.callPackage ./geosans-light.nix { };
  shimmering-fonts = final.callPackage ./fonts.nix { };

  arcaea-ptt-data = final.callPackage ./cc-data.nix { };
  shimmeringmoon = final.callPackage ./shimmeringmoon.nix { };
}
