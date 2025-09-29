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

  extraSrcs = [
    "treefmt.toml"
  ];

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
