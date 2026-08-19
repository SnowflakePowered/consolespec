//! Builds and inspects the definition archive that `consolespec` ships.
//!
//! `cargo archive build` after editing anything under `definitions/`, and
//! commit the result alongside the change. `cargo archive check` is the CI
//! guard that the committed archive still matches the tree.

use std::{fs, path::PathBuf, process::ExitCode};

use consolespec_build::{
    ARCHIVE_FILE_NAME, Archive, Error, Result,
    archive::{DEFAULT_LEVEL, write},
    source,
};

const USAGE: &str = "\
usage: consolespec-archive <build|check|stats> [options]

  build    compile the definition tree into an archive
  check    verify that the committed archive matches the definition tree
  stats    report what an archive is made of

options:
  --definitions <dir>   definition tree (default: <workspace>/definitions)
  --archive <file>      archive path (default: <workspace>/consolespec/definitions.csa)
  --level <n>           zstd level for `build` (default: 19)
";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("consolespec-archive: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let command = arguments.first().cloned().unwrap_or_default();
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the crate sits in a workspace")
        .to_path_buf();
    let mut definitions = workspace.join("definitions");
    let mut archive = workspace.join("consolespec").join(ARCHIVE_FILE_NAME);
    let mut level = DEFAULT_LEVEL;

    let mut index = 1;
    while let Some(flag) = arguments.get(index) {
        let value = arguments
            .get(index + 1)
            .ok_or_else(|| Error::new(format!("{flag} needs a value")))?;
        match flag.as_str() {
            "--definitions" => definitions = PathBuf::from(value),
            "--archive" => archive = PathBuf::from(value),
            "--level" => {
                level = value
                    .parse()
                    .map_err(|_| Error::new(format!("`{value}` is not a zstd level")))?;
            }
            other => return Err(Error::new(format!("unknown option `{other}`\n\n{USAGE}"))),
        }
        index += 2;
    }

    match command.as_str() {
        "build" => {
            let bytes = write(&source::read(&definitions)?, level)?;
            fs::write(&archive, &bytes)
                .map_err(|error| Error::new(format!("{}: {error}", archive.display())))?;
            println!(
                "wrote {} ({:.2} MiB)",
                archive.display(),
                bytes.len() as f64 / (1024.0 * 1024.0)
            );
            Ok(())
        }
        "check" => {
            let expected = source::read(&definitions)?;
            let found = Archive::open(&archive)?.definitions()?;
            // The models are compared rather than the bytes: zstd output can
            // shift between library versions without the contents changing.
            if found == expected {
                println!("{} is up to date", archive.display());
                Ok(())
            } else {
                Err(Error::new(format!(
                    "{} is stale; run `cargo archive build`",
                    archive.display()
                )))
            }
        }
        "stats" => {
            let archive = Archive::open(&archive)?;
            println!("{:>18}  {:>12}  {:>12}", "section", "compressed", "raw");
            for section in archive.sections() {
                println!(
                    "{:>18}  {:>12}  {:>12}",
                    section.kind.name(),
                    section.compressed_len,
                    section.uncompressed_len
                );
            }
            println!("{:>18}  {:>12}", "total", archive.len());
            Ok(())
        }
        "" | "-h" | "--help" | "help" => {
            print!("{USAGE}");
            Ok(())
        }
        other => Err(Error::new(format!("unknown command `{other}`\n\n{USAGE}"))),
    }
}
