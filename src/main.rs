use std::fs::File;
use std::io::{ErrorKind, Read, Seek};
use std::path::PathBuf;
use clap::Parser;
use unconcat_gz::{list_contents, uncompress_all, unconcatenate_files, Progress};

#[derive(Parser)]
struct Opt {
	/// Overwrite existing files
	#[clap(short, long)]
	force: bool,

	/// Split into multiple .gz files instead of decompressing
	#[clap(short, long)]
	split_only: bool,

	/// Output directory for deconcatenated files
	#[clap(short, long, parse(from_os_str))]
	output_directory: PathBuf,

	/// concatenated gzip input file
	#[clap(name = "FILE", parse(from_os_str))]
	path: PathBuf,
}

fn process(opt: Opt) -> std::io::Result<()> {
	let input = opt.path;
	let outdir = opt.output_directory;
	let overwrite = opt.force;

	let mut gzfile = File::open(&input)?;

	if opt.split_only {
		// Uncompress to /dev/null to get a list of all files
		let files = list_contents(&mut gzfile, |_| {})?;

		// Split according to the file list
		unconcatenate_files(&mut gzfile, &files, &outdir, overwrite, |p| match p {
			Progress::FileBegin { name, .. } => eprint!("{}: ", name),
			Progress::FileDone { .. } => eprintln!("OK."),
			Progress::FileFailed { error } => {
				if error.kind() == ErrorKind::AlreadyExists {
					eprintln!("ERROR: {} (use -f to overwrite).", error);
				} else {
					eprintln!("ERROR: {}.", error);
				}
			}
			_ => {}
		})?;
	} else {
		uncompress_all(&mut gzfile, &outdir, overwrite, |p| match p {
			Progress::FileBegin { name, .. } => eprint!("{}: ", name),
			Progress::FileDone { .. } => eprintln!("OK."),
			_ => {}
		})?;
	}

	let mut buf = [0u8; 1];
	if let (Ok(pos), Ok(i)) = (gzfile.stream_position(), gzfile.read(&mut buf)) {
		if i > 0 {
			eprintln!(
				"WARNING: {} contains non-gzip data starting at byte {}.",
				input.display(),
				pos
			);
		}
	}

	Ok(())
}

fn main() {
	let opt = Opt::parse();
	let result = process(opt);
	if let Err(e) = result {
		if e.kind() == ErrorKind::AlreadyExists {
			eprintln!("ERROR: {} (use -f to overwrite).", e);
		} else {
			eprintln!("ERROR: {}.", e);
		}
		eprintln!("Aborted.");
		std::process::exit(1);
	}
}
