use clap::Parser;
use std::fs::File;
use std::io::{ErrorKind, Read, Seek};
use std::path::PathBuf;
use unconcat_gz::{escape_cc, list_contents, uncompress_all, unconcatenate_files, Progress};

#[derive(Parser)]
struct Opt {
	/// Overwrite existing files
	#[clap(short, long)]
	force: bool,

	/// Decompressing all files (default)
	#[clap(short, long, group = "input")]
	_decompress: bool,

	/// Split into multiple .gz files instead of decompressing
	#[clap(short, long, group = "input")]
	split_only: bool,

	/// List all contained files instead of decompressing
	#[clap(short, long, group = "input")]
	list_only: bool,

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

	if opt.list_only {
		// Uncompress to /dev/null to list of files
		list_contents(&mut gzfile, |p| match p {
			Progress::FileBegin { name, .. } => println!("{}", escape_cc(name)),
			_ => {}
		})?;
	} else if opt.split_only {
		// Uncompress to /dev/null to get a list of all files
		let files = list_contents(&mut gzfile, |_| {})?;

		// Split according to the file list
		unconcatenate_files(&mut gzfile, &files, &outdir, overwrite, |p| match p {
			Progress::FileBegin { name, .. } => eprint!("{}: ", escape_cc(name)),
			Progress::FileDone { .. } => eprintln!("OK."),
			Progress::FileFailed { error } => {
				if error.kind() == ErrorKind::AlreadyExists {
					eprintln!("ERROR: {} (use -f to overwrite).", error);
				} else {
					eprintln!("ERROR: {}.", error);
				}
			}
			Progress::ProgressStep => {}
		})?;
	} else {
		uncompress_all(&mut gzfile, &outdir, overwrite, |p| match p {
			Progress::FileBegin { name, .. } => eprint!("{}: ", escape_cc(name)),
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
