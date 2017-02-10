pub const SECTOR_SIZE: usize = 256;

#[derive(Debug, PartialEq, Eq)]
pub enum DFSError {
	Unknown,
	InvalidValue,
	InputTooSmall(usize),
	InvalidDiscData(usize),
}

mod file_p {
	use std::hash::{Hash, Hasher};

	use dfs::*;
	use support::{AsciiChar, AsciiPrintingChar};

	#[derive(Debug, Eq)]
	pub struct File {
		dir: AsciiPrintingChar,
		name: String,
		load_addr: u32,
		exec_addr: u32,
		file_contents: Vec<u8>,
	}


	impl File {
		pub fn name(&self) -> &str {
			&self.name
		}

		pub fn set_name(&mut self, new_name: &str) {
			self.name = new_name.to_owned();
		}

		pub fn directory(&self) -> char {
			self.dir.clone().into()
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
	use core::cell::RefCell;

	use dfs::*;
	use support;

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
		pub disc_cycle: support::BCD,

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

					support::inject(&mut buf, &src[0x000..0x008]).unwrap();
					support::inject(&mut buf[8..], &src[0x100..0x104]).unwrap();
				}

				// Upper bit must not be set
				if let Some(bit7_set) = buf.iter().position(|&n| (n & 0x80) != 0) {
					return Err(DFSError::InvalidDiscData(bit7_set));
				}

				let name_len = buf.iter().take_while(|&&b| b >= 32u8).count();

				// Guaranteed ASCII at this point
				unsafe { String::from_utf8_unchecked(buf[..name_len].to_vec()) }
			};

			let num_catalogue_entries = {
				const OFFSET : usize = 0x105;
				let raw = src[OFFSET];
				if (raw & 0x07) != 0 { return Err(DFSError::InvalidDiscData(OFFSET)); }

				raw >> 3
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
				try!(support::BCD::from_u8(src[OFFSET])
					.map_err(|e| DFSError::InvalidDiscData(OFFSET)))
			};

			let mut files = HashSet::new();

			let mut disc = Disc {
				disc_name: disc_name,
				files: files,
				boot_option: boot_option,
				disc_cycle: disc_cycle,
			};

			Ok(RefCell::new(disc))
		}
	}

}
pub use dfs::disc_p::*;
