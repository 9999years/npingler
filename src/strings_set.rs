use std::collections::HashSet;
use std::hash::Hasher;

use rustc_hash::FxHasher;

use crate::id_hasher::IdHasher;

/// Set of hashed `(old, new)` comparisons.
/// We only keep the `u64`s to save data and lifetime headaches.
///
/// I do NOT know enough about hashes to write this correctly. Maybe.
pub struct FxStringsDiffSet(HashSet<u64, IdHasher>);

impl FxStringsDiffSet {
    pub fn with_capacity(capacity: usize) -> Self {
        // We're only inserting hashes to the set directly!
        Self(HashSet::with_capacity_and_hasher(
            capacity,
            IdHasher::default(),
        ))
    }

    pub fn insert<IntoIter, Iter, Item>(&mut self, (old, new): (IntoIter, IntoIter)) -> SetAdd
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

pub struct FxStringsSet(HashSet<u64, IdHasher>);

impl FxStringsSet {
    pub fn insert<IntoIter, Iter, Item>(&mut self, items: IntoIter) -> SetAdd
    where
        IntoIter: IntoIterator<Item = Item, IntoIter = Iter>,
        Iter: Iterator<Item = Item> + ExactSizeIterator,
        Item: AsRef<str>,
    {
        let mut hasher = FxHasher::default();
        let iter = items.into_iter();
        hasher.write_usize(iter.len());
        for item in iter {
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

pub struct FxStringDiffSet(HashSet<u64, IdHasher>);

impl FxStringDiffSet {
    pub fn with_capacity(capacity: usize) -> Self {
        // We're only inserting hashes to the set directly!
        Self(HashSet::with_capacity_and_hasher(
            capacity,
            IdHasher::default(),
        ))
    }

    pub fn insert(&mut self, old: impl AsRef<str>, new: impl AsRef<str>) -> SetAdd {
        let mut hasher = FxHasher::default();
        hasher.write(old.as_ref().as_bytes());
        hasher.write(new.as_ref().as_bytes());
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
pub enum SetAdd {
    /// Nothing was added, the item was already present.
    AlreadyPresent,
    /// The item was newly-added.
    New,
}
