{ }:
let
  sources = import ./npins;
  pkgs = import sources.nixpkgs {
    overlays = [
      (import ./nix/overlays/local-pkgs.nix)
      (import ./nix/overlays/lib.nix)
    ];
  };
in
pkgs.npinglerPackages.npingler.overrideAttrs (prev: {
  passthru = (prev.passthru or { }) // {
    inherit pkgs;

    inherit (pkgs)
      cargo
      ;

    inherit (pkgs.npinglerPackages)
      npingler
      shell
      checks
      make-release-commit
      ;
  };
})
