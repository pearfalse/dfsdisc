use core::convert::From;
use std::collections::{HashSet, hash_set};

use dfs::*;
use support::*;
use ascii::{AsciiStr,AsciiString};

/// What a DFS-supporting OS would do with a [`Disc`](./struct.Disc.html)
/// found in the drive during a Shift-BREAK.
#[derive(Debug, PartialEq)]
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

impl BootOption {
	/// Attempts to parse a `BootOption` from its `*OPT 4` value. This
	/// function mimics the interface of the (currently unstable)
	/// [`TryFrom`](https://doc.rust-lang.org/stable/std/convert/trait.TryFrom.html)
	/// trait.
	///
	/// # Errors
	/// * [`DFSError::InvalidValue`](./enum.DFSError.html): The given byte
	/// would not convert to a `BootOption`.
	fn try_from(src : u8) -> Result<BootOption, DFSError> {
		match src {
			0u8 => Ok(BootOption::None),
			1u8 => Ok(BootOption::Load),
			2u8 => Ok(BootOption::Run ),
			3u8 => Ok(BootOption::Exec),
			_   => Err(DFSError::InvalidValue)
		}
	}
}

/// Representation of a single-sided DFS disc.
#[derive(Debug)]
pub struct Disc {
	pub name: AsciiString,
	pub boot_option: BootOption,
	pub cycle: BCD,

	files: HashSet<File>,

}

impl Disc {

	/// Decodes a slice of bytes from a disc image into a `Disc`.
	///
	/// As DFS discs could only reach 200KiB in size, there is no provision
	/// for buffered reading.
	///
	/// # Errors
	/// * [`DFSError::InputTooSmall(usize)`][DFSError]: `src` was too small
	/// to be a valid DFS disc image. The attached `usize` indicates the
	/// minimum correct size in bytes, which is 512.
	/// * [`DFSError::InvalidDiscData(usize)`][DFSError]: `src` did not
	/// decode to a valid DFS disc. The attached `usize` is an offset into
	/// `src` where the offending data was found.
	/// * [`DFSError::DuplicateFileName`][DFSError]: Two files were found
	/// with the same name and directory entry. Whether these two files
	/// to the same on-disc data is not checked.
	///
	/// [DFSError]: ./enum.DFSError.html
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use dfsdisc::dfs;
	/// use std::fs::File;
	/// use std::io::Read;
	///
	/// let mut disc_bytes = Vec::new();
	/// {
	/// 	let mut file = File::open("dfsimage.ssd").unwrap();
	/// 	file.read_to_end(&mut disc_bytes).unwrap();
	/// }
	///
	/// let disc = match dfs::Disc::from_bytes(disc_bytes.as_slice()) {
	/// 	Ok(x) => {
	/// 		x
	/// 	},
	/// 	Err(e) => {
	/// 		println!("Error parsing disc: {:?}", e);
	/// 		return;
	/// 	}
	/// };
	///
	/// println!("Files in {}:", disc.name);
	/// for file in &disc {
	/// 	println!("--> {}", file);
	/// }
	/// ```
	pub fn from_bytes(src: &[u8]) -> Result<Disc, DFSError> {

		// Must have minimum size for two sectors
		if src.len() < (SECTOR_SIZE * 2) {
			return Err(DFSError::InputTooSmall(SECTOR_SIZE * 2))
		}

		let disc_name: AsciiString = {
			let buf = unsafe {
				use core::mem;

				// 12 bytes of u8
				// First 8 come from buf[0x000..0x008]
				// Second 4 come from buf[0x100..0x104]
				// We already know the source is big enough
				let mut b: [u8; 12] = mem::uninitialized();
				b[..8].copy_from_slice(&src[0x000..0x008]);
				b[8..].copy_from_slice(&src[0x100..0x104]);

				b
			};

			let name_len = buf.iter().take_while(|&&b| b > 32u8).count();
			AsciiString::from_ascii(&buf[..name_len]).map_err(|e| {
				let str_pos = e.ascii_error().valid_up_to();
				// Decode index position back to byte offset
				DFSError::InvalidDiscData(if str_pos >= 8 {
					str_pos + 0xf8 // start of second sector; 0x008 -> 0x100
				} else {
					str_pos
				})
			})?
		};

		// Disc sector count calculation. We don't check this against the
		// length of `src`, as it's common to have this value declare all
		// 40 or 80 tracks, for a disc image to then only include the ones
		// containing file data. The source extent _is_ checked per-file.
		{
			const OFFSET : usize = 0x107;
			let upper = ((src[OFFSET - 1] & 3) as u16) << 8;
			let result = (src[OFFSET] as u16) | upper;
			if result < 2 {
				return Err(DFSError::InvalidDiscData(OFFSET));
			}
			result
		};

		let boot_option = (src[0x106] >> 4) & 3;
		let boot_option = try!(BootOption::try_from(boot_option));

		let disc_cycle = {
			const OFFSET : usize = 0x104;
			try!(BCD::from_hex(src[OFFSET])
				.map_err(|_| DFSError::InvalidDiscData(OFFSET)))
		};

		let files = try!(populate_files(src));

		let disc = Disc {
			name: disc_name,
			files: files,
			boot_option: boot_option,
			cycle: disc_cycle,
		};

		Ok(disc)
	}
}

fn populate_files(src: &[u8])
-> Result<HashSet<File>, DFSError> {
	let num_catalogue_entries = {
		const OFFSET : usize = 0x105;
		let raw = src[OFFSET];
		if (raw & 0x07) != 0 { return Err(DFSError::InvalidDiscData(OFFSET)); }

		raw >> 3
	};

	let mut files = HashSet::new();
	files.reserve(num_catalogue_entries as usize);

	for i in 0..num_catalogue_entries {
		// First half: filename, directory name, locked bit
		let offset1 = ((i*8) as usize) + 0x008;
		let offset2 = ((i*8) as usize) + 0x108;
		let name_buf = &src[offset1 .. (offset1 + 7)];

		// Set dir, locked
		let dir = src[offset1 + 7];
		let locked = dir > 0x7f;
		let dir = AsciiPrintingChar::from_u8(dir & 0x7f).unwrap();

		// Guard against stray high bits
		if let Some(pos) = name_buf.iter().position(|&b| b < 0x20 || b >= 0x80) {
			return Err(DFSError::InvalidDiscData(offset1 + pos));
		}

		// Set file name as owned string
		let name_len = name_buf.iter().take_while(|&&b| b != 0x20).count();
		let mut name_vec = Vec::with_capacity(name_len);
		for ch in name_buf.iter().take(name_len) {
			name_vec.push(ch & 0x7f);
		}
		// All bytes in `name` guaranteed to be 0x020–0x7f
		let name = unsafe { String::from_utf8_unchecked(name_vec) };

		let busy_byte = src[offset2 + 6] as u32;

		// Load/Exec
		let load_addr = (u16_from_le(&src[offset2 .. offset2 + 2]) as u32)
			| ((busy_byte << 14) & 0x30000);
		let exec_addr = (u16_from_le(&src[offset2 + 2 .. offset2 + 4]) as u32)
			| ((busy_byte << 10) & 0x30000);

		// File length and start sector
		let file_len = (u16_from_le(&src[offset2 + 4 .. offset2 + 6]) as u32)
			| ((busy_byte << 12) & 0x30000);
		let start_sector = (src[offset2 + 7] as u32)
			| ((busy_byte << 8) & 0x300);

		// Validate data offsets
		let data_start = start_sector * 0x100;
		let data_end = data_start + file_len;
		if data_start < 0x200 {
			return Err(DFSError::InvalidDiscData(offset2 + 7));
		}
		if data_end > (src.len() as u32) {
			return Err(DFSError::InvalidDiscData(offset2 + 6));
		}

		let file_contents = &src[(data_start as usize)..(data_end as usize)];

		let file = File {
			dir: dir.clone(),
			name: name,
			load_addr: load_addr,
			exec_addr: exec_addr,
			locked: locked,
			file_contents: From::from(file_contents)
		};

		if files.contains(&file) {
			return Err(DFSError::DuplicateFileName(
				format!("{}.{}", *dir, &file.name)
				));
		}

		files.insert(file);
	}

	Ok(files)
}

impl<'a> IntoIterator for &'a Disc {
	type Item = &'a File;
	type IntoIter = hash_set::Iter<'a, File>;

	fn into_iter(self) -> Self::IntoIter {
		self.files.iter()
	}
}

#[cfg(test)]
mod test {

	use dfs;
	use support::*;

	#[test]
	fn from_bytes_files_success() {
		let mut src = [0u8; dfs::SECTOR_SIZE * 6];
		src[0..8].copy_from_slice(b"Discname");
		// Three files:
		// $.Small (12 bytes of '1') load 0x1234 exec 0x5678
		// A.Single (256 bytes of '2') load 0x8765 exec 0x4321
		// B.Double (257 bytes of '3') load 0x0111 exec 0x0eee
		src[8..40].copy_from_slice(b"Small\x20\x20$Single\x20ADouble\x20BNEVER\x20\x20C");
		src[0x100..0x108].copy_from_slice(b"\x20\x20\x20\x20\x11\x18\x00\x06");
		src[0x108..0x110].copy_from_slice(b"\x34\x12\x78\x56\x0c\x00\x00\x02");
		src[0x110..0x118].copy_from_slice(b"\x65\x87\x21\x43\x00\x01\x00\x03");
		src[0x118..0x120].copy_from_slice(b"\x11\x01\xee\x0e\x01\x01\x00\x04");
		// Don't parse this file!
		src[0x120..0x128].copy_from_slice(b"\xff\xff\xbb\xbb\x01\x00\x00\x05");

		src[0x200..0x20c].copy_from_slice(&[0x31u8; 12]);
		src[0x300..0x400].copy_from_slice(&[0x32u8; 256]);
		src[0x400..0x501].copy_from_slice(&[0x33u8; 257]);

		let target = dfs::Disc::from_bytes(&src);
		assert!(target.is_ok(), format!("{:?}", target.unwrap_err()));
		let target = target.unwrap();

		// Check cycle count
		assert_eq!(BCD::from_hex(0x11).unwrap(), target.cycle);

		for f in target.into_iter() {
			println!("Found file {:?}", f);
		}

		// Start picking files apart
		let check = |dir: char, name: &str, load: u32, exec: u32, len: usize, byte: u8| {
			println!("Checking {}.{}...", dir, name);
			let file = target.into_iter().find(|&f| {
					*f.dir == dir
				}).unwrap_or_else(|| panic!("No file found in dir '{}'", dir));
			assert_eq!(name, file.name);
			assert_eq!(load, file.load_addr);
			assert_eq!(exec, file.exec_addr);
			assert_eq!(len, file.file_contents.len());
			assert!(file.file_contents.iter().all(|&n| n == byte));
		};

		check('$', "Small" , 0x1234, 0x5678, 12, 0x31);
		check('A', "Single", 0x8765, 0x4321, 256, 0x32);
		check('B', "Double", 0x0111, 0x0eee, 257, 0x33);

		assert_eq!(None, target.into_iter().find(|&f| {
			*f.dir == 'C'
		}));
	}

	#[test]
	fn disc_name() {
		let test_name = b"DiscName?!";
		let buf = disc_buf_with_name(test_name);

		let target = dfs::Disc::from_bytes(&buf);
		assert!(target.is_ok(), format!("returned error {:?}", target.unwrap_err()));

		let target = target.unwrap();
		assert_eq!(test_name, target.name.as_bytes());
	}

	#[test]
	fn disc_name_top_bits_set() {
		let disc_name = ::ascii::AsciiStr::from_ascii(b"DiscName").unwrap();

		for i in 0..8 {
			let mut buf = [0u8; 8];
			buf.copy_from_slice(disc_name.as_str().as_bytes());
			buf[i] |= 0x80; // set a high bit

			let disc_bytes = disc_buf_with_name(&buf);

			let target = dfs::Disc::from_bytes(&disc_bytes).unwrap_err();
			assert_eq!(target, dfs::DFSError::InvalidDiscData(i));
		}

		let disc_bytes = disc_buf_with_name(b"DiscNameAB\xffD");
		let target = dfs::Disc::from_bytes(&disc_bytes);
		assert!(target.is_err());

		let target = target.unwrap_err();
		assert_eq!(dfs::DFSError::InvalidDiscData(0x102), target);

		// a space should be a terminator
		let disc_bytes = disc_buf_with_name(b"DiscName \xff\xff\xff");
		let target = dfs::Disc::from_bytes(&disc_bytes);
		assert!(target.is_ok());
		assert_eq!(target.unwrap().name.as_str(), disc_name.as_str());

	}

	#[test]
	fn boot_types() {
		use dfs::BootOption;

		for (boot_type_int, boot_type) in [
			BootOption::None,
			BootOption::Load,
			BootOption::Run,
			BootOption::Exec
		].iter().enumerate() {
			let mut buf = disc_buf_with_name(b"DiscName");
			buf[0x106] = (boot_type_int as u8) << 4;
			let buf = buf;

			let target = dfs::Disc::from_bytes(&buf);
			assert!(target.is_ok());
			let target = target.unwrap();
			assert_eq!(*boot_type, target.boot_option);
		}
	}

	#[test]
	fn invalid_sector_count() {
		let case = |n| {
			let mut buf = disc_buf_with_name(b"DiscName");
			buf[0x107] = n;
			let buf = buf;

			let target = dfs::Disc::from_bytes(&buf);
			assert!(target.is_err());
			let target = target.unwrap_err();
			assert_eq!(target, dfs::DFSError::InvalidDiscData(0x107));
		};

		case(0);
		case(1);
	}

	fn disc_buf_with_name(name: &[u8]) -> [u8 ; dfs::SECTOR_SIZE * 2] {
		let mut buf = [0u8 ; dfs::SECTOR_SIZE * 2];
		let parts = name.split_at(8);
		buf.copy_from_common_slice(parts.0);
		buf[0x100..].copy_from_common_slice(parts.1);
		buf[0x107] = 2; // sector count
		buf
	}
}