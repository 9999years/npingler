{
  lib,
  newScope,
}:
lib.recurseIntoAttrs (
  lib.makeScope newScope (final: {
    inherit (final) callPackage newScope;

    makePins = final.callPackage ./makePins.nix { };

    makePackages = final.callPackage ./makePackages.nix { };

    makeProfile = final.callPackage ./makeProfile.nix { };
  })
)
