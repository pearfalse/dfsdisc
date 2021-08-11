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
	#[options(help = "dump the contents of a disc image")]
	Probe(ScProbe),
	#[options(help = "build a disc image from source files and a manifest")]
	Build(ScBuild),
	#[options(help = "unpack a disc image into separate files (and a manifest)")]
	Unpack(ScUnpack),
}

#[derive(Debug, Options)]
struct ScProbe {
	#[options()]
	help: bool,

	#[options(free)]
	image_file: OsString,
}

#[derive(Debug, Options)]
struct ScBuild {
	#[options()]
	help: bool,

	#[options(short = "x", long = "manifest")]
	manifest: OsString,

	#[options(free)]
	output_file: OsString,
}

#[derive(Debug, Options)]
struct ScUnpack {
	#[options()]
	help: bool,

	#[options(short = "o", long = "output", help = "output folder")]
	output: OsString,

	#[options(free)]
	image_file: OsString,
}

fn main() {
	let args = CliArgs::parse_args_default_or_exit();
	let r = match args.command {
		Some(Subcommand::Probe(ref probe)) => sc_probe(&*probe.image_file),
		Some(Subcommand::Unpack(ref unpack)) => sc_unpack(&*unpack.image_file, &*unpack.output),
		Some(Subcommand::Build(_)) => {
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
enum CliError {
	InputTooLarge,
	Io(io::Error),
	BadImage(dfs::DFSError),
}

impl From<io::Error> for CliError {
	fn from(src: io::Error) -> Self {
	    Self::Io(src)
	}
}


fn read_image(path: &OsStr) -> Result<Vec<u8>, CliError> {
	let mut data = Vec::new();

	if path == "-" {
		let stdin = io::stdin();
		stdin.lock().read_to_end(&mut data)
			.map_err(CliError::Io)?;
	} else {
		File::open(path).map_err(CliError::Io)
		.and_then(|mut f| {
			let file_len = f.metadata().map_err(CliError::Io)?.len();
			if file_len > dfs::MAX_DISC_SIZE as u64 {
				return Err(CliError::InputTooLarge);
			}
			f.read_to_end(&mut data).map_err(CliError::Io)
		})?;
	}

	Ok(data)
}


fn sc_probe(image_path: &OsStr) -> Result<(), CliError> {
	let image_data = read_image(image_path)?;

	let disc = dfs::Disc::from_bytes(&image_data)
		.map_err(CliError::BadImage)?;

	println!("Opened disc {}", disc.name());
	println!("Files:");
	for file in disc.files() {
		println!("{}", file);
	}
	Ok(())
}

fn sc_unpack(image_path: &OsStr, target: &OsStr) -> Result<(), CliError> {
	use std::fs;
	use std::io::Write;
	use ascii::{AsciiChar,AsciiStr};

	let separator = AsciiChar::from_ascii(std::path::MAIN_SEPARATOR).unwrap();

	fs::DirBuilder::new()
		.recursive(true)
		.create(target)
		.map_err(CliError::Io)?;

	std::env::set_current_dir(target)?;

	let image_data = read_image(image_path)?;
	let disc = dfs::Disc::from_bytes(&image_data)
		.map_err(CliError::BadImage)?;

	let dirs: std::collections::HashSet<dfsdisc::support::AsciiPrintingChar>
		= disc.files().map(|f| f.dir()).collect();

	for dir in dirs {
		let as_path = [dir.as_byte()];
		std::fs::create_dir(unsafe {
			// SAFETY: array is always populated with an ASCII subset byte
			&*(&as_path[..] as *const [u8] as *const str)
		})?;
	}

	let mut file_path_buf = arrayvec::ArrayVec::<AsciiChar, 9>::new(); // 9 == 7 of file + dir + SEPARATOR
	for file in disc.files() {
		file_path_buf.clear();
		file_path_buf.push(*file.dir());
		file_path_buf.push(separator);
		file_path_buf.extend(file.name().as_slice().iter().copied());

		fs::File::create(<&AsciiStr>::from(&*file_path_buf).as_str())
			.and_then(|mut f| f.write_all(file.content()))
			.map_err(CliError::Io)?;
	}

	Ok(())
}
