# Read `Cargo.toml` directly from the original (unfiltered) source path rather
# than via `"${src}"`. Interpolating the filtered `lib.fileset.toSource` result
# produces a string carrying store-path context, which recent Nix insists on
# realising before `readFile` will read it. During a release that store path is
# never built, so the read fails with "path ... did not exist in the store
# during evaluation". Reading from the plain path avoids forcing src into the
# store at all.
{ src }: builtins.fromTOML (builtins.readFile (src.origSrc + "/Cargo.toml"))
