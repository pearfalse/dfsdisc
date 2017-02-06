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
