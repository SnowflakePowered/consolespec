# consolespec

This Rust workspace contains:

- [`consolespec`](consolespec), the compile-time console specification library.
- [`consolespec-build`](consolespec-build), the definition archive format that `consolespec` ships and expands.
- [`consolespec-mtree`](consolespec-mtree), the MTREE parser used to compile partition specifications.

The definitions themselves live outside the crates:

- `definitions/inputspec` and `definitions/machinespec` — TOML documents.
- `definitions/partitionspec` — mtree listings of what each firmware installs.
- `schema/` — editor schemas for the TOML documents.
- `scripts/` — the tooling that extracts listings from firmware images.

## The definition archive

`definitions/` is 78 MiB of text, so it is not what gets published. Instead
`consolespec-build` compiles the tree into `consolespec/definitions.csa`, a
4.5 MiB archive with interned path components and deduplicated directory
entries, and `consolespec`'s build script expands that at compile time.

After changing anything under `definitions/`, rebuild the archive and commit
it with the change:

```
cargo archive build     # recompile consolespec/definitions.csa
cargo archive check     # verify the committed archive matches definitions/
cargo archive stats     # report what the archive is made of
```

`cargo archive check` is also a plain test, so `cargo test -p consolespec-build
--features compile` catches an archive that has fallen behind the tree.
