use std::hash::BuildHasher;
use std::hash::Hash;
use std::hash::Hasher;

use rustc_hash::FxBuildHasher;
use rustc_hash::FxHashSet;
use rustc_hash::FxHasher;
use serde_json::map::IntoIter;

use crate::nix::Derivation;
use crate::nix::Nix;

pub fn diff_derivations(nix: &Nix, drv1: &Derivation, drv2: &Derivation) -> miette::Result<String> {
    todo!()
}

enum DiffEvent<'d> {
    Enter(&'d Derivation),
    ArgsAlreadyCompared,
    ArgDiff(diff::Result<&'d str>),
}

struct DiffState<'d> {
    nix: &'d Nix,
    old: &'d Derivation,
    new: &'d Derivation,
    /// Set of hashed `(old, new)` comparisons.
    /// We only keep the `u64`s to save data and lifetime headaches.
    arg_comparisons: FxStringsSet,
}

impl<'d> DiffState<'d> {
    fn diff_one(&mut self, old: &'d Derivation, new: &'d Derivation) -> miette::Result<()> {
        if old.path == new.path {
            return Ok(());
        }

        self.diff_args(&old.args, &new.args)?;

        // compare builders (maybe store paths)
        // compare env
        // compare input_drvs (drvs)
        // compare input_srcs (store paths)
        // compare outputs
        // compare system

        todo!()
    }

    fn diff_args(&mut self, old: &[String], new: &[String]) -> miette::Result<()> {
        if old == new {
            return Ok(());
        }

        let hash = FxBuildHasher.hash_one((old, new));

        if self.arg_comparisons.contains(&hash) {
            self.emit(DiffEvent::ArgsAlreadyCompared)?;
            return Ok(());
        }

        for chunk in diff::slice(old, new) {
            match chunk {
                diff::Result::Left(_) => todo!(),
                diff::Result::Both(_, _) => todo!(),
                diff::Result::Right(_) => todo!(),
            }
        }

        Ok(())
    }

    fn emit(&mut self, event: DiffEvent) -> miette::Result<()> {
        todo!()
    }
}

struct FxStringsDiffSet(FxHashSet<u64>);

impl FxStringsDiffSet {
    fn insert<IntoIter, Iter, Item>(&mut self, (old, new): (IntoIter, IntoIter)) -> SetAdd
    where
        IntoIter: IntoIterator<Item = Item, IntoIter = Iter>,
        Iter: Iterator<Item = Item> + ExactSizeIterator,
        Item: AsRef<str>,
    {
        let mut hasher = FxHasher::default();
        for into_iter in [old, new] {
            let iter = into_iter.into_iter();
            hasher.write_usize(iter.len());
            for item in iter {
                hasher.write(item.as_ref().as_bytes());
            }
        }

        let hash = hasher.finish();

        if self.0.insert(hash) {
            SetAdd::New
        } else {
            SetAdd::AlreadyPresent
        }
    }
}

struct FxStringsSet(FxHashSet<u64>);

impl FxStringsSet {
    fn insert(&mut self, items: impl IntoIterator<Item = impl AsRef<str>>) -> SetAdd {
        let mut hasher = FxHasher::default();
        for item in items {
            hasher.write(item.as_ref().as_bytes());
        }
        let hash = hasher.finish();

        if self.0.insert(hash) {
            SetAdd::New
        } else {
            SetAdd::AlreadyPresent
        }
    }
}

/// Was an item added to a set?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SetAdd {
    /// Nothing was added, the item was already present.
    AlreadyPresent,
    /// The item was newly-added.
    New,
}
