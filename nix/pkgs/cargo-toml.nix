{ src }: builtins.fromTOML (builtins.readFile "${src}/Cargo.toml")
