{
  stdenv,
  npingler,
  src,
  rustPlatform,
  cargo,
  rustc,
}:
{
  name,
  checkPhase,
  checkInputs ? [ ],
  derivationArgs ? { },
}:
stdenv.mkDerivation (
  {
    name = "${name}-check";

    inherit src;
    inherit (npingler) cargoDeps;

    nativeBuildInputs = [
      rustPlatform.cargoSetupHook
    ]
    ++ (derivationArgs.nativeBuildInputs or [ ]);

    checkInputs = [
      cargo
      rustc
    ]
    ++ checkInputs;

    phases = [
      "unpackPhase"
      "checkPhase"
      "installPhase"
    ];

    inherit checkPhase;
    doCheck = true;

    installPhase = ''
      touch $out
    '';
  }
  // builtins.removeAttrs derivationArgs [
    "checkInputs"
    "nativeBuildInputs"
  ]
)
