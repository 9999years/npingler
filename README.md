# npingler

`npingler` is a Nix profile manager intended for use with [`npins`][npins]. A
non-trivial implementation of @lf-'s [`flakey-profile`][flakey-profile], split
off of my earlier (Flake-based) [`home-mangler`][home-mangler].

`npingler` is configured with a Nix expression in `~/.config/npingler/default.nix`:

```nix
let
  npins-sources = import ./npins;
  pkgs = import npins-sources.nixpkgs {
    overlays = [
      (final: prev: {
        inherit npins-sources;

        npingler-lib = final.callPackage "${npins-sources.npingler}/lib" { };
      })
    ];
  };
in
{
  npingler = {
    # By default, npingler uses the attr matching your hostname.
    grandiflora = pkgs.npingler-lib.makeProfile {
      pins = {
        # A map of names to `source` derivations. These get pinned in the `nix
        # registry` so that (e.g.) `nix repl nixpkgs` uses the same version of
        # `nixpkgs` as your profile, and also in your Nix channels, so that
        # `nix-shell -p hello` uses the same version as well.
        nixpkgs = npins-sources.nixpkgs;
      };

      # Install `git` in your profile:
      paths = [
        pkgs.git
      ];
    };
  };
}
```

Switch to the new configuration with `npingler switch`. Use `--dry-run` for a preview.

Note that with [`flake-compat`][flake-compat], you can use `npingler` with a
Flake-based setup (although the `npingler update` command won't do anything).

[npins]: https://github.com/andir/npins
[flakey-profile]: https://github.com/lf-/flakey-profile
[home-mangler]: https://github.com/home-mangler/home-mangler
[flake-compat]: https://git.lix.systems/lix-project/flake-compat
