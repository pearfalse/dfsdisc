//! Types and conversions for DFS disc images.

mod disc;
mod file;

/// Sector size in all known DFS implementations.
pub const SECTOR_SIZE: usize = 256;

/// Largest disc image size in all known DFS implementations.
pub const MAX_DISC_SIZE: usize = 524288;

#[derive(Debug, PartialEq, Eq)]
pub enum DFSError {
	InvalidValue,
	InputTooSmall(usize),
	InputTooLarge(usize),
	InvalidDiscData(usize),
	DuplicateFileName(String),
}

pub use self::disc::*;
pub use self::file::*;
