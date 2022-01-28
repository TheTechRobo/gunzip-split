# gunzip-split - Uncompress concatenated gzip files back into separate files.

[![Crates.io](https://img.shields.io/crates/v/gunzip-split.svg)](https://crates.io/crates/gunzip-split)
[![Documentation](https://docs.rs/gunzip-split/badge.svg)](https://docs.rs/gunzip-split/)
[![License](https://img.shields.io/crates/l/gunzip-split.svg)](https://www.gnu.org/licenses/old-licenses/lgpl-2.1.html)
[![Build Status](https://gitlab.com/cg909/gunzip-split/badges/master/pipeline.svg)](https://gitlab.com/cg909/gunzip-split/-/commits/master)

The `gunzip-split` utility can decompress concatenated gzip files back into separate files.

## Usage

Install with

```sh
$ cargo install gunzip-split
```

And run it like this:

```sh
$ gunzip-split -o ./out/ concatenated.gz
```

Splitting/deconcatenating and listing the filenames of contained compressed files are also supported.

## License

Licensed under GNU Lesser General Public License, Version 2.1 or any later version ([LICENSE](LICENSE).

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, shall be licensed as above.
