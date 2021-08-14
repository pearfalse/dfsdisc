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
			eprintln!("{}", args.self_usage());
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
	use std::borrow::Cow;
	use std::fs;
	use std::io::Write;
	use ascii::{AsciiChar,AsciiStr};
	use xml::{
		writer::events::XmlEvent,
		name::Name as XmlName,
		attribute::Attribute,
		namespace::Namespace,
	};

	const SEPARATOR: AsciiChar = AsciiChar::Slash;
	let root_namespace = Namespace({
		let mut map = std::collections::BTreeMap::new();
		map.insert(String::from(xml::namespace::NS_NO_PREFIX), String::from("http://pearfalse.com/schemas/2021/dfs-manifest"));
		map
	});

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
		std::fs::create_dir_all(dir.as_ascii_str().as_str())?;
	}

	let mut file_path_buf = arrayvec::ArrayVec::<AsciiChar, 9>::new(); // 9 == 7 of file + dir + SEPARATOR
	for file in disc.files() {
		file_path_buf.clear();
		file_path_buf.push(*file.dir());
		file_path_buf.push(SEPARATOR);
		file_path_buf.extend(file.name().as_slice().iter().copied());

		fs::File::create(<&AsciiStr>::from(&*file_path_buf).as_str())
			.and_then(|mut f| f.write_all(file.content()))
			.map_err(CliError::Io)?;
	}

	// create manifest file
	let mut manifest = fs::File::create("manifest.xml")
		.map(|f| xml::writer::EventWriter::new_with_config(f, xml::writer::EmitterConfig {
			indent_string: Cow::Borrowed("\t"),
			perform_indent: true,
			pad_self_closing: false,
			.. Default::default()
		})).map_err(CliError::Io)?;

	// begin manifest
	match (|| {
		manifest.write(XmlEvent::StartDocument {
			version: xml::common::XmlVersion::Version11,
			encoding: Some("UTF-8"),
			standalone: None,
		})?;

		// <dfsdisc>
		let attr_cycle = format!("{}", disc.cycle().into_u8());
		let start_attrs = [
			// hardcoding to 100KiB 40T DFS for now. TODO fix this, obviously
			Attribute::new(XmlName::local("sides"), "1"),
			Attribute::new(XmlName::local("tracks"), "40"),
			Attribute::new(XmlName::local("cycle"), &attr_cycle),
			Attribute::new(XmlName::local("boot"), disc.boot_option().as_str()),
		];
		manifest.write(XmlEvent::StartElement {
			name: XmlName::local("dfsdisc"),
			attributes: Cow::Borrowed(&start_attrs[..]),
			namespace: Cow::Owned(root_namespace),
		})?;

		let ns_empty = xml::namespace::Namespace::empty();
		for file in disc.files() {
			let element_name = match file.exec_addr() & 0xffff {
				0x801f | 0x8023 if file.content().looks_like_basic() => "basic",
				0xffff if file.content().is_mos_text() => "text",
				n if n >= 0x900 && n < 0x8000 => "code",
				_ => "data"
			};

			let dir1 = [file.dir().as_ascii_char()];
			let load_str = format!("{:04x}", file.load_addr());
			let exec_str = format!("{:04x}", file.exec_addr());

			file_path_buf.clear();
			file_path_buf.push(dir1[0]);
			file_path_buf.push(SEPARATOR);
			file_path_buf.extend(file.name().as_slice().iter().copied());

			let file_attrs = [
				Attribute::new(XmlName::local("name"), file.name().as_str()),
				Attribute::new(XmlName::local("dir"), <&AsciiStr>::from(&dir1[..]).as_str()),
				Attribute::new(XmlName::local("src"), <&AsciiStr>::from(&*file_path_buf).as_str()),
				Attribute::new(XmlName::local("load"), &*load_str),
				Attribute::new(XmlName::local("exec"), &*exec_str),
			];

			// <[code|data|text]/>
			manifest.write(XmlEvent::StartElement {
				name: XmlName::local(element_name),
				attributes: Cow::Borrowed(&file_attrs[..]),
				namespace: Cow::Borrowed(&ns_empty),
			})?;
			manifest.write(XmlEvent::end_element())?;
		}

		// </dfsdisc>
		manifest.write(XmlEvent::end_element())?;

		Ok(())
	})() {
		Ok(()) => {},
		Err(xml::writer::Error::Io(e)) => return Err(CliError::Io(e)),
		Err(_e) => panic!("Unexpected XML error: {:?}", _e),
	};

	manifest.into_inner().write_all(b"\n")
		.map_err(CliError::Io)
}

trait FileHeuristics {
	fn is_mos_text(&self) -> bool;
	fn looks_like_basic(&self) -> bool;
}

impl FileHeuristics for [u8] {
	fn is_mos_text(&self) -> bool {
		const CR: u8 = b'\r';
		const PRINTING_LOW : u8 = b'\x21';
		const PRINTING_HIGH: u8 = b'\x7e';
		self.iter().all(|&b| b == CR || (b >= PRINTING_LOW && b <= PRINTING_HIGH))
	}

	fn looks_like_basic(&self) -> bool {
		self.len() >= 2 && [self[0], self[1]] == [0xd, 0x0]
	}
}
