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
    checks.clippy
  ];

  packages = [
    cargo
    rustc
    rust-analyzer
  ];
}
