//! Types and conversions for DFS disc images.

mod disc;
mod file;

/// Sector size in all known DFS implementations.
pub const SECTOR_SIZE: usize = 256;

#[derive(Debug, PartialEq, Eq)]
pub enum DFSError {
	InvalidValue,
	InputTooSmall(usize),
	InvalidDiscData(usize),
	DuplicateFileName(String),
}

pub use self::disc::*;
pub use self::file::*;
