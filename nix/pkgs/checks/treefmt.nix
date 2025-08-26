{
  mkCheck,
  # See: https://github.com/NixOS/nixpkgs/pull/437251
  pkgs,
  nixfmt,
  rustfmt,
  actionlint,
}:

mkCheck {
  name = "treefmt";

  checkInputs = [
    pkgs.treefmt
    nixfmt
    rustfmt
    actionlint
  ];

  checkPhase = ''
    treefmt --ci
  '';
}
