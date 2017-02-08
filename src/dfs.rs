pub const SECTOR_SIZE: usize = 256;

#[derive(Debug)]
pub enum DFSError {
	Unknown,
	InvalidValue,
	InputTooSmall(usize),
	InvalidDiscData(usize),
}

mod file_p {

	use dfs::*;

	#[derive(Debug)]
	pub struct File {
		dir: u8,
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
			self.dir as char
		}

		pub fn set_directory(&mut self, new_dir: u8) -> Result<(), DFSError> {
			if new_dir >= 0x20 && new_dir < 0x7f {
				self.dir = new_dir;
				Ok(())
			}
			else {
				Err(DFSError::InvalidValue)
			}
		}
	}

}
pub use dfs::file_p::*;

mod disc_p {

	use std::collections::HashMap;
	use core::cell::RefCell;

	use dfs::*;
	use support;

	#[derive(Debug)]
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

	#[derive(Debug)]
	pub struct Disc {
		pub disc_name: String,
		pub files: HashMap<u8, File>,
		pub boot_option: BootOption
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

				String::from_utf8_lossy(&buf[..name_len]).into_owned()
			};

			Ok(RefCell::new(Disc {
				disc_name: disc_name,
				files: HashMap::new(),
				boot_option: BootOption::None
			}))
		}
	}

}
pub use dfs::disc_p::*;
