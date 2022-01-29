gunzip-split(1) -- Uncompress concatenated gzip files back into separate files
==============================================================================

## SYNOPSIS

`gunzip-split` [`-d`|`-s`] [`-f`] `-o` _directory_ _file_  
`gunzip-split` `-l` _file_

## DESCRIPTION

`gunzip-split` decompresses a concatenated gzip file back into separate files. When used with `-s`
it splits the file into separate gzip files. The input file is always kept.

## OPTIONS

* `-d`, `--decompress`:
	Decompress. This is the default. For each gzip stream in the input _file_ a corresponding
	expanded file in _directory_ is created. The original file name - if found in the gzip
	metadata - is restored. If no file name is found, "file_`n`" with an incrementing counter `n` is
	used instead.

* `-s`, `--split-only`:
	Split without decompressing. For each gzip stream in the input _file_ a corresponding file in
	_directory_ is created. The original file name - if found in the gzip metadata - with a .gz
	suffix is used. If no file name is found, "file_`n`.gz" with an incrementing counter `n` is
	used instead.

* `-l`, `--list-only`:
	List content files. For each gzip stream in the input _file_ a line containing the
	original file name (or "file_`n`) is printed to standard output.

* `-f`, `--force`:
	Force writing the output files even if a corresponding file already exists. Without this
	option no files are overwritten.

* `-o`, `--output-directory` _directory_:
	Output directory for decompressed/deconcatenated files. It will be created if it doesn't exist.

* `-V`:
	Print version information

* `-h`:
	Print help information

## EXIT STATUS

`gunzip-split` normally exits with status 0. It exits with status 1 if a fatal error occured that
prevented the operation from completing (e.g. non-writable output directory, corrupted archive while
decompressing). The exit status is 2 if required arguments were not provided or conflicting
options were set.

## AUTHOR
Written by Christoph Grenz [christophg at grenz-bonn.de].

## COPYRIGHT
Copyright 2022 Christoph Grenz.

License: GNU LGPL version 2.1 or later
<<https://www.gnu.org/licenses/old-licenses/lgpl-2.1.html>>.

This is free software: you are free to change and redistribute it.
There is NO WARRANTY, to the extent permitted by law.

## SEE ALSO

gzip(1)
