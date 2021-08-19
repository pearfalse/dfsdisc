//! Types and conversions for DFS disc images.

mod disc;
mod file;

/// Sector size in all known DFS implementations.
pub const SECTOR_SIZE: usize = 256;

/// Largest disc image size in all known DFS implementations.
pub const MAX_DISC_SIZE: u64 = 524288;

#[derive(Debug)]
pub enum DFSError {
	InvalidValue,
	InputTooSmall(usize),
	InputTooLarge(usize),
	InvalidDiscData(usize),
	DuplicateFileName(String),
	Io(std::io::Error),
}

impl PartialEq for DFSError {
	fn eq(&self, rhs: &DFSError) -> bool {
		match (self, rhs) {
			(Self::InvalidValue, Self::InvalidValue) => true,
			(Self::InputTooSmall(a), Self::InputTooSmall(b)) => a == b,
			(Self::InputTooLarge(a), Self::InputTooLarge(b)) => a == b,
			(Self::InvalidDiscData(a), Self::InvalidDiscData(b)) => a == b,
			(Self::DuplicateFileName(a), Self::DuplicateFileName(b)) => a == b,
			_ => false,
		}
	}
}

impl From<std::io::Error> for DFSError {
	fn from(src: std::io::Error) -> DFSError {
		DFSError::Io(src)
	}
}

pub use self::disc::*;
pub use self::file::*;
