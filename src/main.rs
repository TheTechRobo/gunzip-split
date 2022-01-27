use std::fs::File;
use std::io::{Read, Seek};
use std::path::PathBuf;
use structopt::StructOpt;
use unconcat_gz::{list_contents, uncompress_all, unconcatenate_files};

#[derive(StructOpt)]
struct Opt {
	/// Overwrite existing files
	#[structopt(short, long)]
	force: bool,

	/// Split into multiple .gz files instead of decompressing
	#[structopt(short, long)]
	split_only: bool,

	/// Output directory for deconcatenated files
	#[structopt(short, long, parse(from_os_str))]
	output_directory: PathBuf,

	/// concatenated gzip input file
	#[structopt(name = "FILE", parse(from_os_str))]
	path: PathBuf,
}

fn process(opt: Opt) -> std::io::Result<()> {
	let input = opt.path;
	let outdir = opt.output_directory;
	let overwrite = opt.force;

	let mut gzfile = File::open(&input)?;

	if opt.split_only {
		let files = list_contents(&mut gzfile)?;
		unconcatenate_files(&mut gzfile, &files, &outdir, overwrite)?;
	} else {
		uncompress_all(&mut gzfile, &outdir, overwrite)?;
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
	let opt = Opt::from_args();
	let result = process(opt);
	if let Err(e) = result {
		eprintln!("ERROR: {}", e);
		eprintln!("Aborted.");
		std::process::exit(1);
	}
}
