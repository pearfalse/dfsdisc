mod file;
mod disc;

pub const SECTOR_SIZE: usize = 256;

#[derive(Debug)]
pub enum DFSError {
	Unknown,
	InvalidValue,
	InputTooSmall(usize),
}
