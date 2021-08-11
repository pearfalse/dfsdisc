use std::hash::{Hash, Hasher};
use std::fmt;

use crate::support::*;

use ascii::AsciiStr;

pub type FileName = AsciiName<7>;

/// A representation of a file in a DFS disc.
///
/// The identity of a `File` (equality, hashing etc.) is determined by the
/// file's name, directory, load address and execution address.
#[derive(PartialEq, Eq)]
pub struct File<'d> {
	/// The DFS directory that this file lives in.
	dir: AsciiPrintingChar,
	/// The name of the file.
	name: FileName,
	/// The address in memory where an OS would load this file.
	load_addr: u32,
	/// The address in memory where, upon loading the file, the OS would begin executing.
	exec_addr: u32,
	/// Whether the file is locked on disc.
	is_locked: bool,
	/// The content of the file.
	content: &'d [u8],
}

impl<'d> File<'d> {
	pub fn new(dir: AsciiPrintingChar, name: FileName, load_addr: u32, exec_addr: u32, is_locked: bool,
		content: &'d [u8]) -> File<'d> {
		File {
			dir,
			name,
			load_addr,
			exec_addr,
			is_locked,
			content,
		}
	}

	pub fn dir(&self) -> AsciiPrintingChar {
		self.dir
	}

	pub fn name(&self) -> &AsciiStr {
		(&*self.name).as_ascii_str()
	}

	pub fn load_addr(&self) -> u32 { self.load_addr }
	pub fn exec_addr(&self) -> u32 { self.exec_addr }
	pub fn is_locked(&self) -> bool { self.is_locked }
	pub fn content(&self) -> &'d [u8] { self.content }

	pub fn lock(&mut self) { self.is_locked = true; }
	pub fn unlock(&mut self) { self.is_locked = false; }

}

impl<'d> Hash for File<'d> {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.dir.hash(state);
		self.name.as_ascii_str().hash(state);
	}
}

impl<'d> fmt::Display for File<'d> {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}.{} (load 0x{:x}, exec 0x{:x}, size 0x{:x})",
			self.dir, self.name,
			self.load_addr, self.exec_addr, self.content().len()
		)
	}
}

impl<'d> fmt::Debug for File<'d> {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "<DFSFile dir={:?} name={:?} \
			load=0x{:x} exec=0x{:x} size=0x{:x}>",
			self.dir, self.name, self.load_addr, self.exec_addr,
			self.content().len()
		)
	}
}
