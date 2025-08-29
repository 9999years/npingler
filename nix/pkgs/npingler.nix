{
  lib,
  stdenv,
  buildPackages,
  rustPlatform,
  installShellFiles,
  src,
  cargo-toml,
}:

let
  emulatorAvailable = stdenv.hostPlatform.emulatorAvailable buildPackages;
  emulator = stdenv.hostPlatform.emulator buildPackages;
in
rustPlatform.buildRustPackage {
  pname = "npingler";
  version = cargo-toml.package.version;

  inherit src;

  cargoHash = "sha256-bP3NO87OxxUzopIud2lr/l+UjcWYDfogco1zfc07QWk=";

  buildFeatures = [ "clap_mangen" ];

  nativeBuildInputs = [
    installShellFiles
  ];

  postInstall = lib.optionalString emulatorAvailable ''
    manpages=$(mktemp -d)
    ${emulator} $out/bin/npingler util generate-man-pages "$manpages"
    for manpage in "$manpages"/*; do
      installManPage "$manpage"
    done

    installShellCompletion --cmd npingler \
      --bash <(${emulator} $out/bin/npingler util generate-completions bash) \
      --fish <(${emulator} $out/bin/npingler util generate-completions fish) \
      --zsh  <(${emulator} $out/bin/npingler util generate-completions zsh)
  '';

  meta = {
    description = "Nix profile manager for use with npins";
    homepage = "https://github.com/9999years/npingler";
    license = lib.licenses.mit;
    maintainers = [
      lib.maintainers._9999years
    ];
    mainProgram = "npingler";
  };
}
