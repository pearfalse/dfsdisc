pub const SECTOR_SIZE: usize = 256;

#[derive(Debug, PartialEq, Eq)]
pub enum DFSError {
	Unknown,
	InvalidValue,
	InputTooSmall(usize),
	InvalidDiscData(usize),
	DuplicateFileName(String),
}

mod file_p {
	use std::hash::{Hash, Hasher};
	use std::fmt;

	use dfs::*;
	use support::{AsciiPrintingChar};

	#[derive(Eq)]
	pub struct File {
		pub dir: AsciiPrintingChar,
		pub name: String,
		pub load_addr: u32,
		pub exec_addr: u32,
		pub locked: bool,
		pub file_contents: Vec<u8>,
	}

	impl Hash for File {
		fn hash<H: Hasher>(&self, state: &mut H) {
			self.dir.hash(state);
			self.name.hash(state);
			self.load_addr.hash(state);
			self.exec_addr.hash(state);
		}
	}

	impl PartialEq for File {
		fn eq(&self, other: &Self) -> bool {
			self.load_addr == other.load_addr &&
			self.exec_addr == other.exec_addr &&
			self.dir == other.dir &&
			self.name == other.name
		}

		fn ne(&self, other: &Self) -> bool {
			self.load_addr != other.load_addr ||
			self.exec_addr != other.exec_addr ||
			self.dir != other.dir ||
			self.name != other.name
		}
	}

	impl fmt::Display for File {
		fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
			write!(f, "{}.{} (load 0x{:x}, exec 0x{:x}, size 0x{:x}",
				*self.dir, self.name,
				self.load_addr, self.exec_addr, self.file_contents.len()
			)
		}
	}

	impl fmt::Debug for File {
		fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
			write!(f, "<DFSFile dir={:?} name={:?} \
				load=0x{:x} exec=0x{:x} size=0x{:x}>",
				self.dir, self.name, self.load_addr, self.exec_addr,
				self.file_contents.len()
			)
		}
	}
}
pub use dfs::file_p::*;

mod disc_p {

	use std::collections::HashSet;
	use core::cell::{RefCell};
	use core::convert::From;
	use std::collections::hash_set;

	use dfs::*;
	use support::*;

	#[derive(Debug, PartialEq)]
	pub enum BootOption {
		None,
		Load,
		Run,
		Exec
	}

	impl From<BootOption> for u8 {
		fn from(src : BootOption) -> u8 {
			match src {
				BootOption::None => {0u8},
				BootOption::Load => {1u8},
				BootOption::Run  => {2u8},
				BootOption::Exec => {3u8},
			}
		}
	}

	impl BootOption {
		fn try_from(src : u8) -> Result<BootOption, DFSError> {
			match src {
				0u8 => Ok(BootOption::None),
				1u8 => Ok(BootOption::Load),
				2u8 => Ok(BootOption::Run ),
				3u8 => Ok(BootOption::Exec),
				_   => Err(DFSError::InvalidValue)
			}
		}
	}

	#[derive(Debug)]
	pub struct Disc {
		pub disc_name: String,
		pub boot_option: BootOption,
		pub disc_cycle: BCD,

		files: HashSet<File>,

	}

	impl Disc {

		pub fn from_bytes(src: &[u8]) -> Result<RefCell<Disc>, DFSError> {

			// Must have minimum size for two sectors
			if src.len() < (SECTOR_SIZE * 2) {
				return Err(DFSError::InputTooSmall(SECTOR_SIZE * 2))
			}

			let disc_name: String = {
				let mut buf: [u8; 12];
				unsafe {
					use core::mem;

					// 12 bytes of u8
					// First 8 come from buf[0x000..0x008]
					// Second 4 come from buf[0x100..0x104]
					// We already know the source is big enough
					buf = mem::uninitialized();

					inject(&mut buf, &src[0x000..0x008]).unwrap();
					inject(&mut buf[8..], &src[0x100..0x104]).unwrap();
				}

				// Upper bit must not be set
				if let Some(bit7_set) = buf.iter().position(|&n| n >= 0x80) {
					let err_pos = if bit7_set >= 8 { bit7_set + 0xf8 } else { bit7_set };
					return Err(DFSError::InvalidDiscData(err_pos));
				}

				let name_len = buf.iter().take_while(|&&b| b >= 32u8).count();

				// Guaranteed ASCII at this point
				unsafe { String::from_utf8_unchecked(buf[..name_len].to_vec()) }
			};

			// Disc sector count calculation. We don't check this against the
			// length of `src`, as it's common to have this value declare all
			// 40 or 80 tracks, for a disc image to then only include the ones
			// containing file data. The source extent _is_ checked per-file.
			{
				const OFFSET : usize = 0x107;
				let upper = ((src[OFFSET - 1] & 3) as u16) << 8;
				let result = (src[OFFSET] as u16) | upper;
				if result < 2 {
					return Err(DFSError::InvalidDiscData(OFFSET));
				}
				result
			};

			let boot_option = (src[0x106] >> 4) & 3;
			let boot_option = try!(BootOption::try_from(boot_option));

			let disc_cycle = {
				const OFFSET : usize = 0x104;
				try!(BCD::from_hex(src[OFFSET])
					.map_err(|_| DFSError::InvalidDiscData(OFFSET)))
			};

			let files = try!(populate_files(src));

			let disc = Disc {
				disc_name: disc_name,
				files: files,
				boot_option: boot_option,
				disc_cycle: disc_cycle,
			};

			Ok(RefCell::new(disc))
		}
	}

	fn populate_files(src: &[u8])
	-> Result<HashSet<File>, DFSError> {
		let num_catalogue_entries = {
			const OFFSET : usize = 0x105;
			let raw = src[OFFSET];
			if (raw & 0x07) != 0 { return Err(DFSError::InvalidDiscData(OFFSET)); }

			raw >> 3
		};

		let mut files = HashSet::new();
		files.reserve(num_catalogue_entries as usize);

		for i in 0..num_catalogue_entries {
			// First half: filename, directory name, locked bit
			let offset1 = ((i*8) as usize) + 0x008;
			let offset2 = ((i*8) as usize) + 0x108;
			let name_buf = &src[offset1 .. (offset1 + 7)];

			// Set dir, locked
			let dir = src[offset1 + 7];
			let locked = dir > 0x7f;
			let dir = AsciiPrintingChar::from_u8(dir & 0x7f).unwrap();

			// Guard against stray high bits
			if let Some(pos) = name_buf.iter().position(|&b| b < 0x20 || b >= 0x80) {
				return Err(DFSError::InvalidDiscData(offset1 + pos));
			}

			// Set file name as owned string
			let name_len = name_buf.iter().take_while(|&&b| b != 0x20).count();
			let mut name_vec = Vec::with_capacity(name_len);
			for ch in name_buf.iter().take(name_len) {
				name_vec.push(ch & 0x7f);
			}
			// All bytes in `name` guaranteed to be 0x020–0x7f
			let name = unsafe { String::from_utf8_unchecked(name_vec) };

			let busy_byte = src[offset2 + 6] as u32;

			// Load/Exec
			let load_addr = (u16_from_le(&src[offset2 .. offset2 + 2]) as u32)
				| ((busy_byte << 14) & 0x30000);
			let exec_addr = (u16_from_le(&src[offset2 + 2 .. offset2 + 4]) as u32)
				| ((busy_byte << 10) & 0x30000);

			// File length and start sector
			let file_len = (u16_from_le(&src[offset2 + 4 .. offset2 + 6]) as u32)
				| ((busy_byte << 12) & 0x30000);
			let start_sector = (src[offset2 + 7] as u32)
				| ((busy_byte << 8) & 0x300);

			// Validate data offsets
			let data_start = start_sector * 0x100;
			let data_end = data_start + file_len;
			if data_start < 0x200 {
				return Err(DFSError::InvalidDiscData(offset2 + 7));
			}
			if data_end > (src.len() as u32) {
				return Err(DFSError::InvalidDiscData(offset2 + 6));
			}

			let file_contents = &src[(data_start as usize)..(data_end as usize)];

			let file = File {
				dir: dir.clone(),
				name: name,
				load_addr: load_addr,
				exec_addr: exec_addr,
				locked: locked,
				file_contents: From::from(file_contents)
			};

			if files.contains(&file) {
				return Err(DFSError::DuplicateFileName(
					format!("{}.{}", *dir, &file.name)
					));
			}

			files.insert(file);
		}

		Ok(files)
	}

	impl<'a> IntoIterator for &'a Disc {
		type Item = &'a File;
		type IntoIter = hash_set::Iter<'a, File>;

		fn into_iter(self) -> Self::IntoIter {
			self.files.iter()
		}
	}
}
pub use dfs::disc_p::*;

#[cfg(test)]
mod test_disc {

	use dfs;
	use support;


	#[test]
	fn from_bytes_files_success() {
		let mut src = [0u8; dfs::SECTOR_SIZE * 6];
		support::inject(&mut src[0..8], b"Discname").unwrap();
		// Three files:
		// $.Small (12 bytes of '1') load 0x1234 exec 0x5678
		// A.Single (256 bytes of '2') load 0x8765 exec 0x4321
		// B.Double (257 bytes of '3') load 0x0111 exec 0x0eee
		support::inject(&mut src[8..40], b"Small\x20\x20$Single\x20ADouble\x20BNEVER\x20\x20C").unwrap();
		support::inject(&mut src[0x100..0x108], b"\x20\x20\x20\x20\x11\x18\x00\x06").unwrap();
		support::inject(&mut src[0x108..0x110], b"\x34\x12\x78\x56\
			\x0c\x00\x00\x02").unwrap();
		support::inject(&mut src[0x110..0x118], b"\x65\x87\x21\x43\
			\x00\x01\x00\x03").unwrap();
		support::inject(&mut src[0x118..0x120], b"\x11\x01\xee\x0e\
			\x01\x01\x00\x04").unwrap();
		// Don't parse this file!
		support::inject(&mut src[0x120..0x128], b"\xff\xff\xbb\xbb\
			\x01\x00\x00\x05").unwrap();

		support::inject(&mut src[0x200..0x20c], &[0x31u8; 12]).unwrap();
		support::inject(&mut src[0x300..0x400], &[0x32u8; 256]).unwrap();
		support::inject(&mut src[0x400..0x501], &[0x33u8; 257]).unwrap();

		let target = dfs::Disc::from_bytes(&src);
		assert!(target.is_ok(), format!("{:?}", target.unwrap_err()));
		let target = target.unwrap().into_inner();

		// Check cycle count
		assert_eq!(support::BCD::from_hex(0x11).unwrap(), target.disc_cycle);

		for f in target.into_iter() {
			println!("Found file {:?}", f);
		}

		// Start picking files apart
		let check = |dir: char, name: &str, load: u32, exec: u32, len: usize, byte: u8| {
			println!("Checking {}.{}...", dir, name);
			let file = target.into_iter().find(|&f| {
					*f.dir == dir
				}).unwrap_or_else(|| panic!("No file found in dir '{}'", dir));
			assert_eq!(name, file.name);
			assert_eq!(load, file.load_addr);
			assert_eq!(exec, file.exec_addr);
			assert_eq!(len, file.file_contents.len());
			assert!(file.file_contents.iter().all(|&n| n == byte));
		};

		check('$', "Small" , 0x1234, 0x5678, 12, 0x31);
		check('A', "Single", 0x8765, 0x4321, 256, 0x32);
		check('B', "Double", 0x0111, 0x0eee, 257, 0x33);

		assert_eq!(None, target.into_iter().find(|&f| {
			*f.dir == 'C'
		}));
	}

	#[test]
	fn disc_name() {
		let test_name = b"DiscName?!";
		let buf = disc_buf_with_name(test_name);

		let target = dfs::Disc::from_bytes(&buf);
		assert!(target.is_ok(), format!("returned error {:?}", target.unwrap_err()));

		let target = target.unwrap().into_inner();
		assert_eq!(test_name, target.disc_name.as_bytes());
	}

	#[test]
	fn disc_name_top_bits_set() {
		let disc_name = b"DiscName";

		for i in 0..8 {
			let mut buf = [0u8; 8];
			support::inject(&mut buf, disc_name).unwrap();
			buf[i] |= 0x80; // set a high bit

			let disc_bytes = disc_buf_with_name(&buf);

			let target = dfs::Disc::from_bytes(&disc_bytes);
			assert!(target.is_err());
			let target = target.unwrap_err();
			assert!(match target {
				dfs::DFSError::InvalidDiscData(at_point) => {
					assert_eq!(i, at_point);
					true
				},
				_ => false
			});
		}

		let disc_bytes = disc_buf_with_name(b"DiscNameAB\xffD");
		let target = dfs::Disc::from_bytes(&disc_bytes);
		assert!(target.is_err());

		let target = target.unwrap_err();
		assert_eq!(dfs::DFSError::InvalidDiscData(0x102), target);
	}

	#[test]
	fn boot_types() {
		use dfs::BootOption;

		for (boot_type_int, boot_type) in [
			BootOption::None,
			BootOption::Load,
			BootOption::Run,
			BootOption::Exec
		].iter().enumerate() {
			let mut buf = disc_buf_with_name(b"DiscName");
			buf[0x106] = (boot_type_int as u8) << 4;
			let buf = buf;

			let target = dfs::Disc::from_bytes(&buf);
			assert!(target.is_ok());
			let target = target.unwrap().into_inner();
			assert_eq!(*boot_type, target.boot_option);
		}
	}

	#[test]
	fn invalid_sector_count() {
		let case = |n| {
			let mut buf = disc_buf_with_name(b"DiscName");
			buf[0x107] = n;
			let buf = buf;

			let target = dfs::Disc::from_bytes(&buf);
			assert!(target.is_err());
			let target = target.unwrap_err();
			assert_eq!(target, dfs::DFSError::InvalidDiscData(0x107));
		};

		case(0);
		case(1);
	}

	fn disc_buf_with_name(name: &[u8]) -> [u8 ; dfs::SECTOR_SIZE * 2] {
		let mut buf = [0u8 ; dfs::SECTOR_SIZE * 2];
		support::inject(&mut buf, &name[..8]).unwrap();
		support::inject(&mut buf[0x100..], &name[8..]).unwrap();
		buf[0x107] = 2; // sector count
		buf
	}
}
