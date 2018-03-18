extern crate dfsdisc;
use dfsdisc::dfs;

#[macro_use]
extern crate clap;
use clap::{App, Arg, SubCommand};

use std::io;
use std::io::Read;
use std::fs::File;

fn main() {

	let args = App::new(crate_name!())
		.about("Perform operations with Acorn DFS disc images")
		.version(crate_version!())
		.author(crate_authors!())
		.subcommand(SubCommand::with_name("probe")
			.about("Interactively probes a disc image")
			.arg(Arg::with_name("image-file")
				.help("The disc image to load (use '-' for stdin)")
				.required(true)
				.index(1)
			)
		)
		.get_matches();

	if let Some(subargs) = args.subcommand_matches("probe") {
		let disc_image = subargs.value_of("image-file").unwrap();
		match sc_probe(disc_image) {
			Ok(()) => { },
			Err(x) => println!("Error: {:?}", x)
		};
	}
}

#[derive(Debug)]
enum ScProbeError {
	InputTooLarge,
	Io(io::Error),
	BadImage(dfs::DFSError),
}

fn sc_probe(image_path: &str) -> Result<(), ScProbeError> {
	let mut data = Vec::new();

	if image_path == "-" {
		let stdin = io::stdin();
		stdin.lock().read_to_end(&mut data)
			.map_err(ScProbeError::Io)?;
	} else {
		File::open(image_path).map_err(ScProbeError::Io)
		.and_then(|mut f| {
			let file_len = f.metadata().map_err(ScProbeError::Io)?.len();
			if file_len > dfs::MAX_DISC_SIZE as u64 {
				return Err(ScProbeError::InputTooLarge);
			}
			f.read_to_end(&mut data).map_err(ScProbeError::Io)
		})?;
	}

	let disc = dfs::Disc::from_bytes(&data)
		.map_err(|e| ScProbeError::BadImage(e))?;

	println!("Opened disc {}", disc.name);
	println!("Files:");
	for file in disc.files() {
		println!("{}", file);
	}
	Ok(())
}