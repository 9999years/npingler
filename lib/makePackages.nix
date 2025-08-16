{ buildEnv }:

args:

buildEnv (
  {
    name = "npingler-packages";
    extraOutputsToInstall = [ "man" ];
  }
  // args
)
