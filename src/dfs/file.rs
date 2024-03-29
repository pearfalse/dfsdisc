use std::borrow::{Borrow,Cow};
use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::fmt;

use crate::support::*;

use ascii::AsciiStr;

pub type FileName = AsciiName<7>;

/// A representation of a file in a DFS disc.
///
/// The identity of a `File` (equality, hashing etc.) is determined by the
/// file's name and directory.
#[derive(PartialEq, Eq)]
pub struct File<'d> {
	/// The name of the file, including directory.
	name: Key,
	/// The address in memory where an OS would load this file.
	load_addr: u32,
	/// The address in memory where, upon loading the file, the OS would begin executing.
	exec_addr: u32,
	/// Whether the file is locked on disc.
	is_locked: bool,
	/// The content of the file.
	content: Cow<'d, [u8]>,
}

impl<'d> File<'d> {
	pub fn new(name: FileName, dir: AsciiPrintingChar,
		load_addr: u32, exec_addr: u32,
		is_locked: bool,
		content: Cow<'d, [u8]>) -> File<'d> {
		File {
			name: Key::new(name, dir),
			load_addr,
			exec_addr,
			is_locked,
			content,
		}
	}

	pub fn dir(&self) -> AsciiPrintingChar {
		self.name.dir
	}

	pub fn name(&self) -> &AsciiStr {
		self.name.name.as_ascii_str()
	}

	pub fn set_name(&mut self, new_name: &AsciiPrintingStr) -> Result<(), AsciiNameError> {
		match AsciiName::<7>::try_from(new_name) {
			Ok(n) => { self.name.name = n; Ok(()) },
			Err(e) => Err(e),
		}
	}

	pub fn load_addr(&self) -> u32 { self.load_addr }
	pub fn exec_addr(&self) -> u32 { self.exec_addr }
	pub fn is_locked(&self) -> bool { self.is_locked }
	pub fn content<'s>(&'s self) -> &'s [u8] where 'd: 's {
		self.content.borrow()
	}

	pub fn lock(&mut self) { self.is_locked = true; }
	pub fn unlock(&mut self) { self.is_locked = false; }


	pub(super) fn key(&self) -> &Key { &self.name }

}

impl<'d> fmt::Display for File<'d> {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}.{} (load 0x{:x}, exec 0x{:x}, size 0x{:x})",
			self.name.dir, self.name.name,
			self.load_addr, self.exec_addr, self.content().len()
		)
	}
}

impl<'d> fmt::Debug for File<'d> {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "<DFSFile dir={:?} name={:?} \
			load=0x{:x} exec=0x{:x} size=0x{:x}>",
			self.name.dir, self.name.name, self.load_addr, self.exec_addr,
			self.content().len()
		)
	}
}

impl<'d> Hash for File<'d> {
	fn hash<H: Hasher>(&self, state: &mut H) { self.name.hash(state); }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct Key {
	pub name: AsciiName<7>,
	pub dir: AsciiPrintingChar,
}

impl<'d> Borrow<Key> for File<'d> {
	fn borrow(&self) -> &Key { &self.name }
}

impl Key {
	pub(super) fn new(name: AsciiName<7>, dir: AsciiPrintingChar) -> Self {
		Self { name, dir }
	}
}

impl Hash for Key {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.dir.hash(state);
		self.name.as_ascii_str().hash(state);
	}
}

impl PartialOrd for Key {
	fn partial_cmp(&self, b: &Key) -> Option<Ordering> { Some(self.cmp(b)) }
}

impl Ord for Key {
	fn cmp(&self, b: &Key) -> Ordering {
		match self.dir.cmp(&b.dir) {
			Ordering::Equal => (*self.name).cmp(&*b.name),
			ne => ne,
		}
	}
}
