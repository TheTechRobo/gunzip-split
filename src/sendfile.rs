use crate::CHUNK_SIZE;
use std::cmp::min;
use std::fs::File;
use std::io::{Error, ErrorKind, Read, Result, Seek, SeekFrom, Write};
use std::ops::{Bound, Range, RangeBounds};
use std::sync::atomic::{AtomicBool, Ordering::Relaxed, Ordering::Release};

#[cfg(unix)]
use libc::{off64_t, sendfile64};
#[cfg(unix)]
use std::os::unix::io::AsRawFd;

#[cfg(unix)]
static SENDFILE_COPY_FAILS: AtomicBool = AtomicBool::new(false);

#[inline]
fn convert_range(range: impl RangeBounds<u64>) -> Option<(u64, u64)> {
	let start = match range.start_bound() {
		Bound::Unbounded => 0,
		Bound::Included(i) => *i,
		Bound::Excluded(i) => *i + 1,
	};

	let length = match range.end_bound() {
		Bound::Unbounded => Some(0),
		Bound::Included(i) => i.checked_sub(start).map(|i| i.saturating_add(1)),
		Bound::Excluded(i) => i.checked_sub(start),
	};

	length.map(|length| (start, length))
}

#[cfg(unix)]
fn sendfile_native(file: &mut File, offset: u64, length: usize, output: &mut File) -> Result<u64> {
	let mut offset: off64_t = offset as off64_t;

	let written = unsafe { sendfile64(output.as_raw_fd(), file.as_raw_fd(), &mut offset, length) };

	if written == -1 {
		Err(Error::last_os_error())
	} else {
		Ok(written as u64)
	}
}

fn sendfile_emulated(
	file: &mut File,
	offset: u64,
	length: usize,
	output: &mut File,
) -> Result<u64> {
	file.seek(SeekFrom::Start(offset))?;
	let mut pending = if length > 0 { length } else { usize::MAX };
	let mut written = 0;
	if pending == 0 {
		return Ok(0);
	}

	let mut buffer = vec![0; min(CHUNK_SIZE, pending)];

	while pending > 0 {
		let max_read = min(pending, CHUNK_SIZE);
		let count = match file.read(&mut buffer[..max_read]) {
			Ok(c) => c,
			Err(e) => return if written > 0 { Ok(written) } else { Err(e) },
		};

		if count == 0 {
			return Ok(written);
		}

		let count = match output.write(&buffer[..count]) {
			Ok(c) if c == count => c,
			Ok(c) => {
				if let Err(e) = file.seek(SeekFrom::Current(c as i64 - count as i64)) {
					return if written > 0 {
						Ok(written + c as u64)
					} else {
						Err(e)
					};
				};
				c
			}
			Err(e) => return if written > 0 { Ok(written) } else { Err(e) },
		};

		pending -= count;
		written += count as u64;
	}

	Ok(written)
}

pub fn sendfile(
	file: &mut File,
	range: impl RangeBounds<u64>,
	output: &mut File,
) -> Result<Option<Range<u64>>> {
	let (offset, mut length) = match convert_range(range) {
		Some(v) => v,
		None => return Ok(None),
	};

	let transfer_size: usize = length.try_into().unwrap_or(usize::MAX);

	if length == 0 {
		length = file.metadata()?.len().saturating_sub(offset);
	}

	#[cfg(unix)]
	// Fast path for unixoid systems
	if !SENDFILE_COPY_FAILS.load(Relaxed) {
		match sendfile_native(file, offset, transfer_size, output) {
			Ok(written) => {
				return if written == 0 || written >= length {
					Ok(None)
				} else {
					Ok(Some(offset + written..offset + length))
				}
			}
			Err(e) => match e.kind() {
				// Interruption -> try again later
				ErrorKind::Interrupted => return Ok(Some(offset..offset + length)),
				// Definitely an error where switching to read+write doesn't help
				ErrorKind::WouldBlock
				| ErrorKind::NotFound
				| ErrorKind::PermissionDenied
				| ErrorKind::TimedOut
				| ErrorKind::OutOfMemory => return Err(e),
				// Everything else switches to read+copy for now
				_ => SENDFILE_COPY_FAILS.store(true, Release),
			},
		}
	}

	match sendfile_emulated(file, offset, transfer_size, output) {
		Ok(written) => {
			if written == 0 || written >= length {
				Ok(None)
			} else {
				Ok(Some(offset + written..offset + length))
			}
		}
		Err(e) => match e.kind() {
			ErrorKind::Interrupted => Ok(Some(offset..offset + length)),
			_ => Err(e),
		},
	}
}
