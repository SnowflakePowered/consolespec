//! The compressed definition archive that `consolespec` ships and expands.
//!
//! The definition tree in this repository is dominated by mtree partition
//! listings: 78 MiB of text, almost all of it repeated, because the same
//! directory entry appears in every firmware revision that contains it.
//! Publishing that tree verbatim is what made the `consolespec` crate large,
//! so the crate instead carries one archive built by [`source`] and
//! [`archive::write`], and its build script expands it through
//! [`Archive`].
//!
//! Shrinking the tree is a matter of naming each thing once. Path components
//! are interned and paths are stored as a parent-plus-component trie, entries
//! are deduplicated across every spec, and digests, sizes, and link targets
//! live in their own tables that entries reference by index. Every table is
//! then delta-encoded column by column so that zstd sees long runs of small
//! numbers rather than interleaved records. What survives is the digest bytes
//! themselves, which are incompressible by construction.
//!
//! Sections are compressed independently so a build that does not ask for
//! partition specs never pays to decode them.

#![forbid(unsafe_code)]

pub mod archive;
pub mod model;
#[cfg(feature = "compile")]
pub mod source;

mod error;

pub use archive::Archive;
pub use error::{Error, Result};
pub use model::{Definitions, DirEntry, DirEntryKind, Document, PartitionSpec};

/// The archive file name that `consolespec` expects beside its build script.
pub const ARCHIVE_FILE_NAME: &str = "definitions.csa";
