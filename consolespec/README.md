# consolespec

A compile-time database of console machine and input specifications. Runtime
lookups read immutable generated tables through build-time indexes and perform
no parsing or filesystem I/O.

Partition mtree files are parsed and validated when the definition archive is
built, not when this crate is compiled. Enable `partition-specs` to access the
compiled [`PartitionSpec`](https://docs.rs/consolespec/latest/consolespec/machine/struct.PartitionSpec.html)
and [`DirEntry`](https://docs.rs/consolespec/latest/consolespec/machine/struct.DirEntry.html)
views through `Partition::specs`. Enable `partition-spec-digests` to include
MD5, SHA-1, and SHA-256 data. Without `partition-specs` the trees are never
decompressed, so the default build only pays for the documents.

The definition sources are not part of this crate. They live at the root of
the [repository](https://github.com/SnowflakePowered/consolespec) and are
compiled into `definitions.csa` by `consolespec-build`, which the build script
expands. See the workspace README for how to rebuild it.

TODO:
* complete 3DS mtree skeleton
* complete WiiU mtrees
* complete PS4/PS5 mtrees
* complete xbox mtrees
