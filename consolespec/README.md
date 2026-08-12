# consolespec

Partition mtree files are parsed and validated at build time. Enable
`partition-specs` to access compiled [`PartitionSpec`](https://docs.rs/consolespec/latest/consolespec/machine/struct.PartitionSpec.html)
and [`DirEntry`](https://docs.rs/consolespec/latest/consolespec/machine/struct.DirEntry.html)
views through `Partition::specs`. Enable `partition-spec-digests` to include
MD5, SHA-1, and SHA-256 data.

TODO:
* complete 3DS mtree skeleton
* complete WiiU mtrees
* complete PS4/PS5 mtrees
* complete xbox mtrees
