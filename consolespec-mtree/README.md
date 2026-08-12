# consolespec-mtree

A library and command line interface for the parsing and writing of [ALPM-MTREE] files used in **A**rch **L**inux **P**ackage **M**anagement (ALPM).

This is a history-preserving fork of [`alpm-mtree` 0.3.3] for use by the
`consolespec` project.

[`alpm-mtree` 0.3.3]: https://crates.io/crates/alpm-mtree/0.3.3

## Examples

### Library

```rust
use consolespec_mtree::mtree::v2::parse_mtree_v2;

let data = r#"#mtree
/set mode=644 uid=0 gid=0 type=file
./some_file time=1700000000.0 size=1337 sha256digest=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef
./some_link type=link link=some_file time=1700000000.0
./some_dir type=dir time=1700000000.0
"#.to_string();

assert!(parse_mtree_v2(data).is_ok());
```

### CLI

Validate an `.MTREE` file.

```shell
consolespec-mtree validate path/to/file
```

Parse an `.MTREE` file and output its contents as structured data.

```shell
consolespec-mtree format ~/.cache/alpm/testing/packages/core/argon2-20190702-6-x86_64/.MTREE --output-format json --pretty
```

## Features

- `cli` adds dependencies required for the `consolespec-mtree` command line interface.
- `creation` adds library support for the creation of [ALPM-MTREE] files (enabled by default).
- `_winnow-debug` enables the `winnow/debug` feature, which shows the exact parsing process of winnow.

## License

This project can be used under the terms of the [Apache-2.0] or [MIT].
Contributions to this project, unless noted otherwise, are automatically licensed under the terms of both of those licenses.

[ALPM-MTREE]: https://alpm.archlinux.page/specifications/ALPM-MTREE.5.html
[Apache-2.0]: https://spdx.org/licenses/Apache-2.0.html
[MIT]: https://spdx.org/licenses/MIT.html
