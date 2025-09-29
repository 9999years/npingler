{
  lib,
  extraSrcs ? [ ],
}:
let
  root = ../../.;
in
lib.fileset.toSource {
  root = root;
  fileset = lib.fileset.intersection (lib.fileset.fromSource (lib.sources.cleanSource root)) (
    lib.fileset.unions (
      [
        (root + "/Cargo.toml")
        (root + "/Cargo.lock")
        (root + "/src")
        (root + "/config.toml")
      ]
      ++ builtins.map (path: root + ("/" + path)) extraSrcs
    )
  );
}
