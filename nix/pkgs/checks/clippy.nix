{
  mkCheck,
  # See: https://github.com/NixOS/nixpkgs/pull/437251
  pkgs,
}:

mkCheck {
  name = "clippy";

  checkInputs = [
    pkgs.clippy
  ];

  checkPhase = ''
    cargo clippy --all-targets -- --deny warnings
  '';
}
