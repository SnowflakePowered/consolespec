//! Guards the committed archive against the definition tree drifting out from
//! under it.
//!
//! Only meaningful inside this repository: a published `consolespec-build` has
//! neither the definition tree nor the archive beside it, so the test reports
//! that it found nothing to compare rather than failing.
#![cfg(feature = "compile")]

use std::path::PathBuf;

use consolespec_build::{ARCHIVE_FILE_NAME, Archive, source};

#[test]
fn the_committed_archive_matches_the_definition_tree() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the crate sits in a workspace")
        .to_path_buf();
    let definitions = workspace.join("definitions");
    let archive = workspace.join("consolespec").join(ARCHIVE_FILE_NAME);
    if !definitions.is_dir() || !archive.is_file() {
        eprintln!("no definition tree beside the crate; nothing to compare");
        return;
    }

    let expected = source::read(&definitions).expect("definition tree");
    let found = Archive::open(&archive)
        .expect("committed archive")
        .definitions()
        .expect("archive contents");
    assert_eq!(
        found.inputs.len(),
        expected.inputs.len(),
        "{ARCHIVE_FILE_NAME} is stale; run `cargo archive build`"
    );
    assert_eq!(
        found.partition_specs.len(),
        expected.partition_specs.len(),
        "{ARCHIVE_FILE_NAME} is stale; run `cargo archive build`"
    );
    assert!(
        found == expected,
        "{ARCHIVE_FILE_NAME} is stale; run `cargo archive build`"
    );
}
