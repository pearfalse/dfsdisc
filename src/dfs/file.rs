use std::hash::{Hash, Hasher};
use std::fmt;

use support::{AsciiPrintingChar};

use ascii::{AsciiStr,AsciiString};

/// A representation of a file in a DFS disc.
///
/// The identity of a `File` (equality, hashing etc.) is determined by the
/// file's name, directory, load address and execution address.
#[derive(Eq)]
pub struct File {
	/// The DFS directory that this file lives in.
	pub dir: AsciiPrintingChar,
	/// The name of the file.
	pub name: String, // TODO: constrain to 7 chars, either with a new wrapper type or hiding the field
	/// The address in memory where an OS would load this file.
	pub load_addr: u32,
	/// The address in memory where, upon loading the file, the OS would begin executing.
	pub exec_addr: u32,
	/// Whether the file is locked on disc.
	pub locked: bool,
	/// The contents of the file.
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
		write!(f, "{}.{} (load 0x{:x}, exec 0x{:x}, size 0x{:x})",
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
