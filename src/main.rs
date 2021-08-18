use dfsdisc::dfs;
use dfsdisc::support::*;

use std::borrow::Cow;
use std::io;
use std::io::Read;
use std::ffi::{OsStr,OsString};
use std::fs::File;
use std::path::Path;
use std::str::FromStr;

use gumdrop::Options;

const XML_NAMESPACE: &'static str = "http://pearfalse.com/schemas/2021/dfs-manifest";

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
	Pack(ScPack),
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
struct ScPack {
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
		Some(Subcommand::Pack(ref pack)) => sc_pack(pack.manifest.as_ref(), pack.output_file.as_ref()),
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
	XmlParseError(xml::reader::Error),
	ManifestError(Cow<'static, str>),
}

impl<O> From<CliError> for Result<O, CliError> {
	fn from(src: CliError) -> Self { Err(src) }
}

impl From<io::Error> for CliError {
	fn from(src: io::Error) -> Self {
	    Self::Io(src)
	}
}

impl From<dfs::DFSError> for CliError {
	fn from(src: dfs::DFSError) -> Self {
		Self::BadImage(src)
	}
}

impl From<xml::reader::Error> for CliError {
	fn from(src: xml::reader::Error) -> Self {
		Self::XmlParseError(src)
	}
}


type CliResult = Result<(), CliError>;


macro_rules! warn {
	($format:literal $(, $arg:expr)*) => {
		eprintln!(concat!("warning: ", $format) $(, &($arg))*)
	};
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

fn sc_unpack(image_path: &OsStr, target: &OsStr) -> CliResult {
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
		map.insert(String::from(xml::namespace::NS_NO_PREFIX), String::from(XML_NAMESPACE));
		map
	});

	fs::DirBuilder::new()
		.recursive(true)
		.create(target)
		?;

	std::env::set_current_dir(target)?;

	let image_data = read_image(image_path)?;
	let disc = dfs::Disc::from_bytes(&image_data)?;

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
			?;
	}

	// create manifest file
	let mut manifest = fs::File::create("manifest.xml")
		.map(|f| xml::writer::EventWriter::new_with_config(f, xml::writer::EmitterConfig {
			indent_string: Cow::Borrowed("\t"),
			perform_indent: true,
			pad_self_closing: false,
			.. Default::default()
		}))?;

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
			Attribute::new(XmlName::local("name"), disc.name().as_str()),
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

	manifest.into_inner().write_all(b"\n")?;
	Ok(())
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


fn sc_pack(manifest_path: &Path, image_path: &Path) -> CliResult {
	use xml::reader::XmlEvent;

	macro_rules! dfs_error {
		($const:literal) => {
			CliError::ManifestError(Cow::Borrowed($const))
		};
		($fmt:literal $(, $arg:expr)*) => {
			CliError::ManifestError(Cow::Owned(format!(
				$fmt $(, $arg)*
			)))
		};
	}

	let root = std::fs::canonicalize(manifest_path)
		.map_err(CliError::Io)?;

	// open and parse manifest file
	let mut reader = File::open(&*root)
		.map(xml::EventReader::new)?;

	// CD to path folder
	std::env::set_current_dir(root.parent().unwrap())?;

	// load files

	// - attempt to get root element
	match reader.next()? {
		XmlEvent::StartDocument { version: _, encoding: _, standalone: _ } => {},
		_ => return Err(dfs_error!("expected XML document start")),
	};
	let mut disc = match reader.next()? {
		XmlEvent::StartElement {name: _, attributes, namespace} => {
			match namespace.get(xml::namespace::NS_NO_PREFIX) {
				Some(XML_NAMESPACE) => {},
				Some(_other) => warn!("document has unexpected XML namespace; wanted '{}'", XML_NAMESPACE),
				None => warn!("document has no XML namespace; expected '{}'", XML_NAMESPACE),
			};

			let mut disc = dfs::Disc::new();

			if let Some(name) = attributes.local_attr("name") {
				let ap_name = AsciiPrintingStr::try_from_str(name)
					.map_err(|_| dfs_error!("invalid disc name"))?;
				disc.set_name(ap_name).map_err(|e| dfs_error!(
					"disc name has non-printing or non-ASCII character at position {}", e.position()
					))?;
			}

			if let Some(cycle) = attributes.local_attr("cycle") {
				*disc.cycle_mut() = u8::from_str(cycle).ok()
					.and_then(|r#u8| BCD::from_hex(r#u8).ok())
					.ok_or_else(|| dfs_error!("incorrect cycle count; not valid 2-digit BCD"))?;
			}

			if let Some(boot_option) = attributes.local_attr("boot") {
				match dfs::BootOption::from_str(boot_option) {
					Ok(bo) => *disc.boot_option_mut() = bo,
					Err(_) => return Err(dfs_error!("invalid boot option"))
				};
			}

			Ok(disc)
		},
		_ => Err(dfs_error!("missing <dfsdisc> start element")),
	}?;

	// create files
	loop {
		match reader.next()? {
			XmlEvent::StartElement { name, attributes, namespace: _ } => {
				let element_name = match name.borrow().local_name {
					n @ "text" | n @ "basic" | n @ "data" | n @ "code" => n,
					o => return Err(dfs_error!("unrecognised element '{}'", o)),
				};

				let dir = match attributes.local_attr("dir")
				.map(AsciiPrintingChar::try_from_str) {
					Some(Ok(c)) => Ok(c),
					None => Ok(AsciiPrintingChar::DOLLAR),
					Some(Err(_)) => Err(dfs_error!("dir is not a printing ascii char")),
				}?;

				let name = match attributes.local_attr("name")
				.map(|d| AsciiName::<7>::try_from(d.as_bytes())) {
					Some(Ok(n)) => Ok(n),
					None => Err(dfs_error!("filename must be specified")),
					Some(Err(_)) => Err(dfs_error!("could not convert file name")),
				}?;

				let parse_addr = |addr_name: &str| -> Result<u32, CliError> {
					match attributes.local_attr(addr_name).map(|s| u32::from_str_radix(s, 16)) {
						Some(Ok(u)) => Ok(u),
						Some(Err(_)) => Err(dfs_error!("couldn't parse {} address", addr_name)),
						None => Err(dfs_error!("{} address is missing", addr_name)),
					}
				};
				let load_addr = parse_addr("load")?;
				let exec_addr = parse_addr("exec")?;

				let src_path = attributes.local_attr("src")
					.ok_or_else(|| dfs_error!("src attribute is missing"))?;
				let mut src = File::open(src_path)?;
				if src.metadata().map(|m| m.len()).unwrap_or(u64::MAX) > dfs::MAX_DISC_SIZE {
					return Err(dfs_error!("file '{}' is too big to fit", src_path))?;
				}
				// get file contents
				let contents = {
					let mut c = Vec::new();
					src.read_to_end(&mut c)?;
					c
				};

				match disc.add_file(dfs::File::new(name, dir, load_addr, exec_addr,
				false, /* TODO */
				Cow::Owned(contents))) {
					Ok(None) => {},
					Ok(Some(old)) => warn!("replacing existing file '{}.{}'", old.dir(), old.name()),
					Err(failed) => return Err(
						dfs_error!("file '{}.{}' was specified twice", failed.dir(), failed.name())
					),
				};

				match reader.next()? {
					XmlEvent::EndElement { name } if name.local_name == element_name => {},
					o => return Err(dfs_error!("uncrecognised element {:?}, was expecting </{}>",
						o, element_name)),
				};
			},
			XmlEvent::EndElement {name} if name.local_name == "dfsdisc" => break,
			XmlEvent::Whitespace(_) | XmlEvent::Comment(_) => {}, // who care
			other => return Err(dfs_error!("unrecognised element: {:?}", other)),
		};
	}

	// write it out to target
	eprintln!("File was parsed, files were read. no disc image for you yet, sorry");

	Ok(())
}

trait AttributesExt {
	type Attr: ?Sized;

	fn local_attr(&self, local_name: &str) -> Option<&Self::Attr>;
}

impl AttributesExt for [xml::attribute::OwnedAttribute] {
	type Attr = str;

	fn local_attr(&self, local_name: &str) -> Option<&Self::Attr> {
		let target = xml::name::Name::local(local_name);
		self.iter().find(move |attr| attr.name.borrow() == target)
			.map(|attr| attr.value.as_str())
	}
}
