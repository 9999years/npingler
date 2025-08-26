{
  mkShell,
  cargo,
  rustc,
  rust-analyzer,
  checks,
}:

mkShell {
  name = "npingler-shell";

  inputsFrom = [
    checks.treefmt
  ];

  packages = [
    cargo
    rustc
    rust-analyzer
  ];
}
