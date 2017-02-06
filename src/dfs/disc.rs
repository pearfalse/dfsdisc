use std::collections::HashMap;

use dfs::*;
use dfs::file::*;

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
	pub fn from_bytes(src: &[u8]) -> Result<Disc, DFSError> {
		
		// Must have minimum size for two sectors
		if src.len() < (SECTOR_SIZE * 2) {
			return Err(DFSError::InputTooSmall(SECTOR_SIZE * 2))
		}

		let disc_name: String;
		{
			let mut buf: [u8; 12];
			unsafe {
				use core::mem;
				use std::ptr::copy_nonoverlapping;

				// 12 bytes of u8
				// First 8 come from buf[0x000..0x008]
				// Second 4 come from buf[0x100..0x104]
				// We already know the buffer is big enough
				buf = mem::uninitialized();
			
				let src_p = &src[0] as *const u8;
				let dst_p = &mut buf[0] as *mut u8;
				copy_nonoverlapping(src_p, dst_p, 8);
				copy_nonoverlapping(src_p.offset(0x100), dst_p.offset(8), 4);
			}

			let name_len = buf.into_iter().take_while(|&&b| b >= 32u8).count();
			disc_name = String::from_utf8_lossy(&buf[..name_len]).into_owned();
		}

		Ok(Disc {
			disc_name: disc_name,
			files: HashMap::new(),
			boot_option: BootOption::None
		})
	}
}
