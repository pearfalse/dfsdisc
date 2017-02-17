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

	use dfs::*;
	use support::{AsciiChar, AsciiPrintingChar};

	#[derive(Debug, Eq)]
	pub struct File {
		pub dir: AsciiPrintingChar,
		pub name: String,
		pub load_addr: u32,
		pub exec_addr: u32,
		pub locked: bool,
		pub file_contents: Vec<u8>,
	}


	impl File {
		pub fn name(&self) -> &str {
			&self.name
		}

		pub fn set_name(&mut self, new_name: &str) {
			self.name = new_name.to_owned();
		}

		pub fn directory(&self) -> char {
			*self.dir
		}

		pub fn set_directory(&mut self, new_dir: u8) -> Result<(), DFSError> {
			self.dir = try!(AsciiPrintingChar::from_u8(new_dir).
				map_err({|_| DFSError::InvalidValue }));
			Ok(())
		}
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

}
pub use dfs::file_p::*;

mod disc_p {

	use std::collections::HashSet;
	use core::cell::{Cell,RefCell};
	use std::rc::{Rc,Weak};
	use core::convert::From;
	use std::iter;
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

		files: HashSet<Rc<File>>,

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
					return Err(DFSError::InvalidDiscData(bit7_set));
				}

				let name_len = buf.iter().take_while(|&&b| b >= 32u8).count();

				// Guaranteed ASCII at this point
				unsafe { String::from_utf8_unchecked(buf[..name_len].to_vec()) }
			};

			let sector_count = {
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
				try!(BCD::from_u8(src[OFFSET])
					.map_err(|e| DFSError::InvalidDiscData(OFFSET)))
			};

			let mut files = try!(populate_files(src));

			let mut disc = Disc {
				disc_name: disc_name,
				files: files,
				boot_option: boot_option,
				disc_cycle: disc_cycle,
			};

			Ok(RefCell::new(disc))
		}
	}

	fn populate_files(src: &[u8])
	-> Result<HashSet<Rc<File>>, DFSError> {
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
			let offset1 = ((i*1) as usize) + 0x008;
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
			let mut name_vec = Vec::with_capacity(name_buf.len());
			for ch in name_buf {
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

			let mut file = Rc::new(File {
				dir: dir.clone(),
				name: name,
				load_addr: load_addr,
				exec_addr: exec_addr,
				locked: locked,
				file_contents: From::from(file_contents)
			});

			let file2 = file.clone();

			if !files.insert(file) {
				return Err(DFSError::DuplicateFileName(
					format!("{}.{}", *file2.dir, file2.name)
					));
			}
		}

		Ok(files)
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
		support::inject(&mut src[8..32], b"\
			Small \x20$\
			Single\x20A\
			Double\x20B\
			").unwrap();
		support::inject(&mut src[0x100..0x108], b"\x20\x20\x20\x20\x11\x18\x00\x06").unwrap();
		support::inject(&mut src[0x108..0x110], b"\x34\x12\x78\x56\
			\x0c\x00\x00\x02").unwrap();
		support::inject(&mut src[0x110..0x118], b"\x65\x87\x21\x43\
			\x00\x01\x00\x03").unwrap();
		support::inject(&mut src[0x118..0x120], b"\x11\x01\xee\x0e\
			\x01\x01\x00\x04").unwrap();

		support::inject(&mut src[0x200..0x20c], &[0x31u8; 12]).unwrap();
		support::inject(&mut src[0x300..0x400], &[0x32u8; 256]).unwrap();
		support::inject(&mut src[0x400..0x501], &[0x33u8; 257]).unwrap();

		let target = dfs::Disc::from_bytes(&src);
		assert!(target.is_ok(), format!("{:?}", target.unwrap_err()));

		// Start picking files apart
		// let file_small
	}
}
