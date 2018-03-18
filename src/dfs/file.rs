use std::borrow::Borrow;
use std::hash::{Hash, Hasher};
use std::fmt;

use support::*;

use ascii::{AsciiStr, AsciiString};

/// A representation of a file in a DFS disc.
///
/// The identity of a `File` (equality, hashing etc.) is determined by the
/// file's name, directory, load address and execution address.
#[derive(PartialEq, Eq)]
pub struct File {
	/// The DFS directory that this file lives in.
	dir: AsciiPrintingChar,
	/// The name of the file.
	name: AsciiString,
	/// The address in memory where an OS would load this file.
	load_addr: u32,
	/// The address in memory where, upon loading the file, the OS would begin executing.
	exec_addr: u32,
	/// Whether the file is locked on disc.
	is_locked: bool,
	/// The content of the file.
	content: Box<[u8]>,
}

impl File {
	pub fn new(dir: AsciiPrintingChar, name: AsciiString, load_addr: u32, exec_addr: u32, is_locked: bool,
		content: Box<[u8]>) -> File {
		File {
			dir: dir,
			name: name,
			load_addr: load_addr,
			exec_addr: exec_addr,
			is_locked: is_locked,
			content: content,
		}
	}

	pub fn dir(&self) -> AsciiPrintingChar {
		self.dir
	}

	pub fn name(&self) -> &AsciiStr {
		self.name.borrow()
	}

	pub fn load_addr(&self) -> u32 { self.load_addr }
	pub fn exec_addr(&self) -> u32 { self.exec_addr }
	pub fn is_locked(&self) -> bool { self.is_locked }
	pub fn content(&self) -> &[u8] { &*self.content }

	pub fn lock(&mut self) { self.is_locked = true; }
	pub fn unlock(&mut self) { self.is_locked = false; }

}

impl Hash for File {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.dir.hash(state);
		self.name.hash(state);
		self.load_addr.hash(state);
		self.exec_addr.hash(state);
	}
}

impl fmt::Display for File {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}.{} (load 0x{:x}, exec 0x{:x}, size 0x{:x})",
			self.dir, self.name,
			self.load_addr, self.exec_addr, self.content().len()
		)
	}
}

impl fmt::Debug for File {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "<DFSFile dir={:?} name={:?} \
			load=0x{:x} exec=0x{:x} size=0x{:x}>",
			self.dir, self.name, self.load_addr, self.exec_addr,
			self.content().len()
		)
	}
}
