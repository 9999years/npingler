{
  writeShellApplication,
  cargo,
  cargo-release,
  git,
  nix-update,
}:
writeShellApplication {
  name = "make-release-commit";

  runtimeInputs = [
    cargo
    cargo-release
    git
    nix-update
  ];

  text = ''
    if [[ -n "''${CI:-}" ]]; then
      git config --local user.email "github-actions[bot]@users.noreply.github.com"
      git config --local user.name "github-actions[bot]"
    fi

    cargo release --version

    cargo release \
      --execute \
      --no-confirm \
      "$@"

    nix-update \
        --no-src \
        --version skip \
        npingler

    git commit -a --amend --no-edit
  '';
}
