use dfsdisc::dfs;

use std::io;
use std::io::Read;
use std::ffi::{OsStr,OsString};
use std::fs::File;

use gumdrop::Options;

#[derive(Debug, Options)]
struct CliArgs {
	#[options(help = "print this")]
	help: bool,

	#[options(command)]
	command: Option<Subcommand>,
}

#[derive(Debug, Options)]
enum Subcommand {
	Probe(ScProbe),
	Build(ScBuild),
	Unpack(ScUnpack),
}

#[derive(Debug, Options)]
struct ScProbe {
	#[options(free)]
	image_file: OsString,
}

#[derive(Debug, Options)]
struct ScBuild {
	#[options(short = "x", long = "manifest")]
	manifest: OsString,

	#[options(free)]
	output_file: OsString,
}

#[derive(Debug, Options)]
struct ScUnpack {
	#[options(short = "x", long = "manifest")]
	manifest: OsString,

	#[options(free)]
	image_file: OsString,
}

fn main() {
	let args = CliArgs::parse_args_default_or_exit();
	let r = match args.command {
		Some(Subcommand::Probe(ref probe)) => sc_probe(&*probe.image_file).map_err(Box::new),
		Some(Subcommand::Build(_) | Subcommand::Unpack(_)) => {
			eprintln!("not implemented, sorry");
			Ok(())
		},
		None => {
			eprintln!("No command specified; run with '-h' or '--help' for guidance");
			std::process::exit(1);
		}
	};

	if let Err(e) = r {
		eprintln!("{:?}", e);
	}
}

#[derive(Debug)]
enum ScProbeError {
	InputTooLarge,
	Io(io::Error),
	BadImage(dfs::DFSError),
}

fn sc_probe(image_path: &OsStr) -> Result<(), ScProbeError> {
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
		.map_err(ScProbeError::BadImage)?;

	println!("Opened disc {}", disc.name());
	println!("Files:");
	for file in disc.files() {
		println!("{}", file);
	}
	Ok(())
}
