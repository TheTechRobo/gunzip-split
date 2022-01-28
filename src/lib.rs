//! Splits concatenated gzip files and decompresses them separately
//!
//! This crate contains the main functionality of the `gunzip-split` utility.

use flate2::bufread::GzDecoder;
use std::borrow::Cow;
use std::fs::{create_dir, rename, File};
use std::io::{BufRead, BufReader, Error, ErrorKind, Read, Result, Seek, SeekFrom, Write};
use std::path::Path;

mod sendfile;

pub(crate) const CHUNK_SIZE: usize = 1 * 1024 * 1024;

#[derive(Debug)]
pub enum Progress<'a> {
	/// Processing of a file starts
	///
	/// - `start`: offset in the input file where processing starts
	/// - `name`: filename (if no metadata is found a dummy name like "file_`counter`" is used)
	FileBegin { start: u64, name: &'a str },
	/// Forward progress happened
	///
	/// Guarantees for the progress callbacks:
	/// - passed at least once for every file that could be successfully processed
	/// - passed directly after an OS signal interrupted a system call.
	ProgressStep,
	/// Processing the current file failed, but this wasn't fatal for the whole operation
	///
	/// - `error`: underlying error
	FileFailed { error: Error },
	/// Current file was processed successfully
	///
	/// - `end`: offset in the input file where processing completed
	FileDone { end: u64 },
}

/// Constituent file information
#[derive(Clone, Debug)]
pub struct FileInfo {
	pub filename: String,
	pub start: u64,
	pub end: u64,
}

/// Replace ASCII control characters with REPLACEMENT CHARACTER
///
/// They may upset terminals when used in e.g. a filename or (in the case of NL) confuse users.
#[doc(hidden)]
#[must_use]
#[inline]
pub fn escape_cc(input: &str) -> Cow<'_, str> {
	if input.find(|c| ('\0'..'\x1f').contains(&c)).is_some() {
		input
			.replace(|c| ('\0'..'\x1f').contains(&c), "\u{FFFD}")
			.into()
	} else {
		input.into()
	}
}

/// Uncompresses one file from the gzip input
///
/// The passed `decoder` must already be positioned at the start offset.
///
/// Any `/`, `\` or NUL byte in filename will be replaced with an underscore to prevent
/// directory traversal attacks.
///
/// # Errors
/// If this function encounters any form of I/O or other error except interruption by a signal,
/// an adequate error variant will be returned. The underlying reader will be positioned near
/// the offset where the error occured.
///
/// Interruption by a signal instead causes `progress_cb` to be called with
/// [`Progress::ProgressStep`].
pub fn uncompress_one<R: BufRead>(
	decoder: &mut GzDecoder<R>,
	output_directory: &Path,
	filename: &str,
	overwrite: bool,
	mut progress_cb: impl FnMut(Progress) -> (),
) -> Result<()> {
	let mut buffer = vec![0; CHUNK_SIZE];
	let filename = filename.replace(['/', '\0', '\\'], "_");
	let tempfile = format!("{}.part", filename);
	let outpath = output_directory.join(&tempfile);
	let finalpath = output_directory.join(&filename);

	// Early bail out if we would overwrite a file
	if !overwrite && (finalpath.exists() || finalpath.is_symlink()) {
		return Err(Error::new(
			ErrorKind::AlreadyExists,
			format!("file {} already exists", escape_cc(&filename)),
		));
	}

	// Decompress the file
	{
		// Create .part file
		let mut outfile = if overwrite {
			File::create(&outpath)?
		} else {
			match File::options().write(true).create_new(true).open(&outpath) {
				Ok(f) => f,
				Err(e) => match e.kind() {
					ErrorKind::AlreadyExists => {
						return Err(Error::new(
							ErrorKind::AlreadyExists,
							format!("file {} already exists", escape_cc(&tempfile)),
						))
					}
					_ => return Err(e),
				},
			}
		};

		// Decompress contents into .part file
		let mut chunk_size = 0;
		while match decoder.read(&mut *buffer) {
			Ok(0) => false,
			Ok(i) => {
				chunk_size = i;
				true
			}
			Err(e) => match e.kind() {
				ErrorKind::Interrupted => true,
				_ => return Err(e),
			},
		} {
			// Write chunk into file
			let mut slice = &mut buffer[..chunk_size];
			while !slice.is_empty() {
				let count = match outfile.write(slice) {
					Ok(c) => c,
					Err(e) => match e.kind() {
						ErrorKind::Interrupted => 0,
						_ => return Err(e),
					},
				};
				slice = &mut slice[count..];
				progress_cb(Progress::ProgressStep);
			}
		}
	}

	// Move to final destination
	if !overwrite && (finalpath.exists() || finalpath.is_symlink()) {
		return Err(Error::new(
			ErrorKind::AlreadyExists,
			format!("file {} already exists", escape_cc(&filename)),
		));
	}
	rename(outpath, finalpath)?;

	Ok(())
}

/// Uncompresses all files from the gzip input
///
/// The passed `file` must be positioned at the beginning of the input.
///
/// # Errors
/// If this function encounters any form of I/O or other error except interruption by a signal,
/// an adequate error variant will be returned. The underlying reader will be positioned near
/// the offset where the error occured.
///
/// Interruption by a signal instead causes `progress_cb` to be called with
/// [`Progress::ProgressStep`].
pub fn uncompress_all(
	file: &mut File,
	output_directory: &Path,
	overwrite: bool,
	mut progress_cb: impl FnMut(Progress) -> (),
) -> Result<()> {
	let mut reader = BufReader::with_capacity(CHUNK_SIZE * 2, file);
	let mut counter: u64 = 1;

	while !reader.fill_buf()?.is_empty() {
		let start = reader.stream_position()?;
		let mut decoder = GzDecoder::new(reader);
		let filename: String = if let Some(header) = decoder.header() {
			header.filename().map_or_else(
				|| format!("file_{}", counter),
				|bytes| String::from_utf8_lossy(bytes).into(),
			)
		} else {
			let mut reader = decoder.into_inner();
			let _ = reader.seek(SeekFrom::Start(start));
			return Ok(());
		};
		counter += 1;
		progress_cb(Progress::FileBegin {
			start,
			name: &filename,
		});

		uncompress_one(
			&mut decoder,
			output_directory,
			&filename,
			overwrite,
			&mut progress_cb,
		)?;
		reader = decoder.into_inner();

		progress_cb(Progress::FileDone {
			end: reader.stream_position()?,
		});
	}
	Ok(())
}

/// Lists all files in the input gzip file
///
/// # Errors
/// If this function encounters any form of I/O or other error except interruption by a signal,
/// an adequate error variant will be returned. The position in `input` is undefined.
///
/// Interruption by a signal instead causes `progress_cb` to be called with
/// [`Progress::ProgressStep`].
pub fn list_contents(
	file: &mut File,
	mut progress_cb: impl FnMut(Progress) -> (),
) -> Result<Vec<FileInfo>> {
	let length = file.metadata()?.len();
	let mut reader = BufReader::with_capacity(CHUNK_SIZE * 2, file);
	let mut buffer = vec![0; CHUNK_SIZE];
	let mut files: Vec<FileInfo> = Vec::with_capacity(2);

	while !reader.fill_buf()?.is_empty() {
		let start = reader.stream_position()?;
		let mut decoder = GzDecoder::new(reader);
		let filename: String = if let Some(header) = decoder.header() {
			header.filename().map_or_else(
				|| format!("file_{}", files.len() + 1),
				|bytes| String::from_utf8_lossy(bytes).into(),
			)
		} else {
			let mut reader = decoder.into_inner();
			let _ = reader.seek(SeekFrom::Start(start));
			return Ok(files);
		};

		progress_cb(Progress::FileBegin {
			start,
			name: &filename,
		});

		while match decoder.read(&mut *buffer) {
			Ok(0) => false,
			Ok(_) => true,
			Err(e) => {
				if e.kind() == ErrorKind::Interrupted {
					true
				} else {
					files.push(FileInfo {
						filename: format!("{}~corrupted", filename),
						start,
						end: length,
					});
					progress_cb(Progress::FileFailed { error: e });
					return Ok(files);
				}
			}
		} {
			progress_cb(Progress::ProgressStep);
			// skip contents
		}

		reader = decoder.into_inner();
		let end = reader.stream_position()?;
		files.push(FileInfo {
			filename,
			start,
			end,
		});
		progress_cb(Progress::FileDone { end });
	}
	Ok(files)
}

/// Copies one gzip file described by `info` from the concatenated gzip input
///
/// The filename will be `info.filename` with `.gz` appended. Any `/`, `\`, or NUL byte in
/// the resulting filename will be replaced with an underscore to prevent directory traversal
/// attacks.
///
/// # Errors
/// If this function encounters any form of I/O or other error except interruption by a signal,
/// an adequate error variant will be returned. The position in `input` is undefined.
///
/// Interruption by a signal instead causes `progress_cb` to be called with
/// [`Progress::ProgressStep`].
pub fn write_one_file(
	input: &mut File,
	info: &FileInfo,
	output_directory: &Path,
	overwrite: bool,
	mut progress_cb: impl FnMut(Progress) -> (),
) -> Result<()> {
	input.seek(SeekFrom::Start(info.start))?;
	let filename = format!("{}.gz", info.filename.replace(['/', '\0', '\\'], "_"));
	let tempfile = format!("{}.part", filename);
	let outpath = output_directory.join(&tempfile);
	let finalpath = output_directory.join(&filename);

	// Early bail out if we would overwrite a file
	if !overwrite && (finalpath.exists() || finalpath.is_symlink()) {
		return Err(Error::new(
			ErrorKind::AlreadyExists,
			format!("file {} already exists", escape_cc(&filename)),
		));
	}

	// Write into .part file
	{
		let mut outfile = if overwrite {
			File::create(&outpath)?
		} else {
			match File::options().write(true).create_new(true).open(&outpath) {
				Ok(f) => f,
				Err(e) => match e.kind() {
					ErrorKind::AlreadyExists => {
						return Err(Error::new(
							ErrorKind::AlreadyExists,
							format!("file {} already exists", escape_cc(&tempfile)),
						))
					}
					_ => return Err(e),
				},
			}
		};
		let mut range = info.start..info.end;
		while let Some(r) = sendfile::sendfile(input, range, &mut outfile)? {
			progress_cb(Progress::ProgressStep);
			range = r;
		}
	}

	// Move to final destination
	if !overwrite && (finalpath.exists() || finalpath.is_symlink()) {
		return Err(Error::new(
			ErrorKind::AlreadyExists,
			format!("file {} already exists", escape_cc(&filename)),
		));
	}
	rename(outpath, finalpath)?;

	Ok(())
}

/// Splits the concatenated gzip input into separate files
///
/// After successfully returning, `input` will be positioned at the end of the last file.
///
/// # Errors
/// If this function cannot ensure the existence of the output directory, an error will be returned.
///
/// Any form of I/O or other error encountered while extracting one file,
/// causes `progress_cb` to be called with [`Progress::FileFailed`].
///
/// Interruption by a signal instead causes `progress_cb` to be called with
/// [`Progress::ProgressStep`].
pub fn unconcatenate_files(
	input: &mut File,
	infos: &[FileInfo],
	output_directory: &Path,
	overwrite: bool,
	mut progress_cb: impl FnMut(Progress) -> (),
) -> Result<()> {
	// Try to create output directory
	if let Err(e) = create_dir(output_directory) {
		if e.kind() != ErrorKind::AlreadyExists {
			return Err(e);
		}
	}

	// Write files
	for info in infos {
		progress_cb(Progress::FileBegin {
			start: info.start,
			name: &info.filename,
		});
		if let Err(e) = write_one_file(input, info, output_directory, overwrite, &mut progress_cb) {
			progress_cb(Progress::FileFailed { error: e });
		} else {
			progress_cb(Progress::FileDone { end: info.end });
		}
	}

	// Seek after the last file for correct error position information about corrupted tail data
	// even if native sendfile was used.
	let len = infos.len();
	if len > 0 {
		let _ = input.seek(SeekFrom::Start(infos[len - 1].end));
	}

	Ok(())
}
