{
  lib,
  linkFarm,
}:

# This logic is mostly copied from `flakey-profile`. Thanks @lf-!
# See: https://github.com/lf-/flakey-profile/blob/243c903fd8eadc0f63d205665a92d4df91d42d9d/lib/pin.nix
pins':

let
  pathOk = item: builtins.match ".*-source$" (toString item) != null;
  pathChecked =
    name: item:
    lib.assertMsg (pathOk item) ''
      Flake registry pin item path must end with -source, due to https://github.com/NixOS/nix/issues/7075.
      Name: ${name}
      Path: ${toString item}

      Consider pinning nixpkgs with `builtins.fetchTarball` with `name` set to "source".
    '';

  pins = builtins.mapAttrs (
    name: value:
    assert pathChecked name value;
    value
  ) pins';
in
{
  inherit pins;
  channels = linkFarm "user-environment" pins;
}
