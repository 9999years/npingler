{
  mkShell,
  cargo,
  rustc,
  rustfmt,
  rust-analyzer,
  treefmt,
  actionlint,
}:

mkShell {
  name = "npingler-shell";

  packages = [
    cargo
    rustc
    rustfmt
    rust-analyzer
    treefmt
    actionlint
  ];
}
