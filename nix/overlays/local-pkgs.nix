final: prev: {
  npinglerPackages = final.lib.packagesFromDirectoryRecursive {
    inherit (final) callPackage newScope;
    directory = ../pkgs;
  };

  npingler = final.npinglerPackages.npingler;
}
