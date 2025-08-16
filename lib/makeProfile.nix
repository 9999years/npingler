{
  makePins,
  makePackages,
}:

{
  pins ? { },
  paths ? { },
  makePackagesArgs ? { },
}:

{
  pins = makePins pins;
  packages = makePackages (
    {
      inherit paths;
    }
    // makePackagesArgs
  );
}
