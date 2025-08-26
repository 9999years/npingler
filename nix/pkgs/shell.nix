{
  mkShell,
  cargo,
  rustc,
  rustfmt,
  rust-analyzer,
}:

mkShell {
  name = "npingler-shell";

  packages = [
    cargo
    rustc
    rustfmt
    rust-analyzer
  ];
}
