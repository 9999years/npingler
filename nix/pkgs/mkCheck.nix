{
  lib,
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
  extraSrcs ? [ ],
  checkInputs ? [ ],
  derivationArgs ? { },
}:
stdenv.mkDerivation (
  {
    name = "${name}-check";

    # Narsty tbqh.
    src = src.override {
      inherit extraSrcs;
    };

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
