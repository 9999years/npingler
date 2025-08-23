#![expect(dead_code)]

use std::fmt::Display;
use std::hash::Hash;
use std::io::Write;
use std::ops::AddAssign;

use miette::Context;
use miette::IntoDiagnostic;
use owo_colors::OwoColorize;
use similar::DiffOp;

use crate::nix::Derivation;
use crate::nix::Nix;
use crate::strings_set::FxStringDiffSet;
use crate::strings_set::FxStringsDiffSet;
use crate::strings_set::SetAdd;

pub fn diff_derivations(nix: &Nix, old: &Derivation, new: &Derivation) -> miette::Result<String> {
    const DERIVATIONS_TO_COMPARE: usize = 2048;
    const RENDERED_DIFF_BYTES: usize = 4096;
    let mut writer = Vec::with_capacity(RENDERED_DIFF_BYTES);
    let mut state = DiffState {
        nix,
        old,
        new,
        builder_comparisons: FxStringDiffSet::with_capacity(DERIVATIONS_TO_COMPARE),
        arg_comparisons: FxStringsDiffSet::with_capacity(DERIVATIONS_TO_COMPARE),
        writer: &mut writer,
        indent: String::new(),
    };
    state.diff_one(old, new)?;
    String::from_utf8(writer)
        .into_diagnostic()
        .wrap_err("derivation diff produced invalid UTF-8. uh oh!")
}

enum DiffEvent<'d> {
    Enter {
        old: &'d Derivation,
        new: &'d Derivation,
    },
    AlreadyCompared {
        description: &'d str,
    },
    /// Cringe variant in my fail data structure.
    /// This duplication feels bad !!
    DiffOwned {
        description: &'d str,
        diff: RenderableDiff<'d, String>,
    },
    DiffBorrowed {
        description: &'d str,
        diff: RenderableDiff<'d, &'d str>,
    },
}

struct DiffState<'d, W> {
    nix: &'d Nix,
    old: &'d Derivation,
    new: &'d Derivation,
    builder_comparisons: FxStringDiffSet,
    arg_comparisons: FxStringsDiffSet,
    indent: String,
    writer: W,
}

impl<'d, W> DiffState<'d, W>
where
    W: Write,
{
    fn diff_one(
        &mut self,
        old: &'d Derivation,
        new: &'d Derivation,
    ) -> miette::Result<DiffNovelty> {
        if old.path == new.path {
            return Ok(DiffNovelty::Boring);
        }

        self.emit(DiffEvent::Enter { old, new })?;

        let mut novelty = DiffNovelty::Boring;

        novelty += self.diff_builders(old.builder.as_str(), new.builder.as_str())?;
        novelty += self.diff_args(&old.args, &new.args)?;

        // compare env
        // compare input_drvs (drvs)
        // compare input_srcs (store paths)
        // compare outputs
        // compare system
        //
        // did you see anything new for a particular derivation-tree? if no, collapse it!
        // this requires bundling up the diff events i guess

        Ok(novelty)
    }

    fn diff_builders(&mut self, old: &str, new: &str) -> miette::Result<DiffNovelty> {
        if old == new {
            return Ok(DiffNovelty::Boring);
        }

        if let SetAdd::AlreadyPresent = self.builder_comparisons.insert(old, new) {
            self.emit(DiffEvent::AlreadyCompared {
                description: "Builders",
            })?;
            return Ok(DiffNovelty::Boring);
        }

        self.emit(DiffEvent::DiffBorrowed {
            description: "Builders",
            diff: RenderableDiff::diff(&[old], &[new]),
        })?;

        Ok(DiffNovelty::Novel)
    }

    fn diff_args(&mut self, old: &[String], new: &[String]) -> miette::Result<DiffNovelty> {
        if old == new {
            return Ok(DiffNovelty::Boring);
        }

        if let SetAdd::AlreadyPresent = self.arg_comparisons.insert((old, new)) {
            self.emit(DiffEvent::AlreadyCompared {
                description: "Builder args",
            })?;
            return Ok(DiffNovelty::Boring);
        }

        self.emit(DiffEvent::DiffOwned {
            description: "Builder args",
            diff: RenderableDiff::diff(old, new),
        })?;

        Ok(DiffNovelty::Novel)
    }

    fn emit(&mut self, event: DiffEvent) -> miette::Result<()> {
        match event {
            DiffEvent::Enter { old, new } => {
                self.line(format!("- {}", old.path).red())?;
                self.line(format!("+ {}", new.path).green())?;
                self.indent.push_str("  ");
            }
            DiffEvent::AlreadyCompared { description } => {
                self.line(format_args!("{} already compared", description).yellow())?;
            }
            DiffEvent::DiffOwned { description, diff } => {
                self.line(format_args!("{} changed:", description).bold())?;
                self.render_diff(&diff)?;
            }
            DiffEvent::DiffBorrowed { description, diff } => {
                self.line(format_args!("{} changed:", description).bold())?;
                self.render_diff(&diff)?;
            }
        }
        Ok(())
    }

    fn line(&mut self, line: impl Display) -> miette::Result<()> {
        writeln!(self.writer, "{}{}", self.indent, line).into_diagnostic()?;
        Ok(())
    }

    fn render_diff(&mut self, diff: &RenderableDiff<'_, impl AsRef<str>>) -> miette::Result<()> {
        // TODO: Customizability.
        const CONTEXT: usize = 3;

        for op in diff.ops.iter().copied() {
            match op {
                DiffOp::Equal {
                    old_index: _,
                    new_index,
                    len,
                } => {
                    let new_i_end = new_index + len;
                    if len > CONTEXT {
                        for new_i in new_index..new_index + CONTEXT {
                            self.line(&format!("  {}", diff.new[new_i].as_ref().dimmed()))?;
                        }
                        for new_i in new_i_end - CONTEXT..new_i_end {
                            self.line(&format!("  {}", diff.new[new_i].as_ref().dimmed()))?;
                        }
                    } else {
                        for new_i in new_index..new_i_end {
                            self.line(&format!("  {}", diff.new[new_i].as_ref().dimmed()))?;
                        }
                    }
                }
                DiffOp::Delete {
                    old_index,
                    old_len,
                    new_index: _,
                } => {
                    for old_i in old_index..old_index + old_len {
                        self.line(&format!("- {}", diff.old[old_i].as_ref().red()))?;
                    }
                }
                DiffOp::Insert {
                    old_index: _,
                    new_index,
                    new_len,
                } => {
                    for new_i in new_index..new_index + new_len {
                        self.line(&format!("+ {}", diff.new[new_i].as_ref().green()))?;
                    }
                }
                DiffOp::Replace {
                    old_index,
                    old_len,
                    new_index,
                    new_len,
                } => {
                    for old_i in old_index..old_index + old_len {
                        self.line(&format!("- {}", diff.old[old_i].as_ref().red()))?;
                    }
                    for new_i in new_index..new_index + new_len {
                        self.line(&format!("+ {}", diff.new[new_i].as_ref().green()))?;
                    }
                }
            }
        }

        Ok(())
    }
}

fn diff_slices<T>(old: &[T], new: &[T]) -> Vec<DiffOp>
where
    T: Eq + Hash + Ord,
{
    similar::capture_diff_slices(similar::Algorithm::Patience, old, new)
}

struct RenderableDiff<'a, T> {
    old: &'a [T],
    new: &'a [T],
    ops: Vec<DiffOp>,
}

impl<'a, T> RenderableDiff<'a, T>
where
    T: AsRef<str> + Hash + Ord,
{
    fn diff(old: &'a [T], new: &'a [T]) -> Self {
        Self {
            old,
            new,
            ops: diff_slices(old, new),
        }
    }
}

/// Was anything "new" seen when running a diff?
#[derive(Debug, Clone, Copy)]
enum DiffNovelty {
    /// Either nothing changed or nothing new changed.
    Boring,
    /// Something new changed.
    Novel,
}

impl AddAssign for DiffNovelty {
    fn add_assign(&mut self, rhs: Self) {
        if let DiffNovelty::Novel = rhs {
            *self = DiffNovelty::Novel;
        }
    }
}
