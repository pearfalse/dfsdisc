use std::borrow::Cow;
use std::convert::TryFrom;
use std::collections::HashSet;
use std::io;
use std::marker::PhantomData;

use ascii::AsciiStr;
use arrayvec::ArrayVec;

use crate::dfs::*;
use crate::support::*;

/// What a DFS-supporting OS would do with a [`Disc`](./struct.Disc.html)
/// found in the drive during a Shift-BREAK.
#[derive(Debug, PartialEq, Clone, Copy, enum_utils::FromStr)]
#[enumeration(case_insensitive)]
#[repr(u8)]
pub enum BootOption {
	None = 0,
	Load = 1,
	Run  = 2,
	Exec = 3,
}

impl BootOption {
	pub fn as_str(self) -> &'static str {
		match self {
			Self::None => "none",
			Self::Load => "load",
			Self::Run  => "run" ,
			Self::Exec => "exec",
		}
	}
}

impl From<BootOption> for u8 {
	fn from(src: BootOption) -> u8 { src as u8 }
}

impl TryFrom<u8> for BootOption {
	type Error = DFSError;

	fn try_from(src: u8) -> Result<BootOption, DFSError> {
		match src {
			0u8 => Ok(BootOption::None),
			1u8 => Ok(BootOption::Load),
			2u8 => Ok(BootOption::Run ),
			3u8 => Ok(BootOption::Exec),
			_   => Err(DFSError::InvalidValue)
		}
	}
}

const MAX_FILES: u8 = 31;
const MAX_SECTORS: u16 = 800; // 10 sectors × 80 tracks

type HeaderSectors = [u8; 0x200];
pub type DiscName = AsciiName<12>;

/// Representation of a single-sided DFS disc.
#[derive(Debug)]
pub struct Disc<'d> {
	_data: PhantomData<&'d [u8]>,

	// TODO: hold tracks count

	name: DiscName,
	boot_option: BootOption,
	cycle: BCD,
	files: HashSet<File<'d>>,
}

impl<'d> Disc<'d> {

	// Basic accessors
	pub fn cycle(&self) -> BCD { self.cycle }
	pub fn cycle_mut(&mut self) -> &mut BCD { &mut self.cycle }
	pub fn increment_cycle(&mut self) {
		let next_cycle = self.cycle.into_u8().wrapping_add(1);
		self.cycle = match BCD::try_new(next_cycle) {
			Ok(bcd) => bcd,
			Err(_) => BCD::C00
		};
	}

	pub fn name(&self) -> &AsciiStr { self.name.as_ascii_str() }
	pub fn set_name(&mut self, new_name: &AsciiPrintingStr) -> Result<(), AsciiNameError> {
		match AsciiName::try_from(new_name) {
			Ok(n) => { self.name = n; Ok(()) },
			Err(e) => Err(e),
		}
	}

	pub fn boot_option(&self) -> BootOption { self.boot_option }
	pub fn boot_option_mut(&mut self) -> &mut BootOption { &mut self.boot_option }

	/// Creates a new, empty DFS disc.
	pub fn new() -> Disc<'d> {
		Disc {
			_data: PhantomData,

			name: DiscName::empty(),
			boot_option: BootOption::None,
			cycle: BCD::C00,
			files: HashSet::new(),
		}
	}

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
	/// with the same name and directory entry. Whether these two files point
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
	/// println!("Files in {}:", disc.name());
	/// for file in disc.files() {
	/// 	println!("--> {}", file);
	/// }
	/// ```
	pub fn from_bytes(src: &'d [u8]) -> Result<Disc<'d>, DFSError> {
		let header_sectors: &HeaderSectors = src.as_min_slice().map_err(|_| DFSError::InputTooSmall(SECTOR_SIZE * 2))?;

		let disc_name = {
			let buf = {
				// 12 bytes of u8
				// First 8 come from buf[0x000..0x008]
				// Second 4 come from buf[0x100..0x104]
				// We already know the source is big enough
				let mut b: [u8; 12] = [0; 12];
				b[..8].copy_from_slice(&header_sectors[0x000..0x008]);
				b[8..].copy_from_slice(&header_sectors[0x100..0x104]);

				b
			};

			let name_len = buf.iter().take_while(|&&b| b > 32u8).count();
			DiscName::try_from(&buf[..name_len]).map_err(|e| {
				let str_pos = e.position();
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
			let upper = ((header_sectors[OFFSET - 1] & 3) as u16) << 8;
			let result = (header_sectors[OFFSET] as u16) | upper;
			if result < 2 {
				return Err(DFSError::InvalidDiscData(OFFSET));
			}
			result
		};

		let boot_option = (header_sectors[0x106] >> 4) & 3;
		let boot_option = BootOption::try_from(boot_option)?;

		let disc_cycle = {
			const OFFSET : usize = 0x104;
			BCD::from_hex(header_sectors[OFFSET])
				.map_err(|_| DFSError::InvalidDiscData(OFFSET))?
		};

		let files = populate_files(src)?;

		let disc = Disc {
			_data: PhantomData,
			name: disc_name,
			files,
			boot_option,
			cycle: disc_cycle,
		};

		Ok(disc)
	}

	pub fn files<'a>(&'a self) -> Files {
		Files(self.files.iter())
	}

	pub fn add_file(&mut self, file: File<'d>) -> Result<Option<File<'d>>, File<'d>> {
		if self.files.len() >= MAX_FILES as usize {
			return Err(file);
		}

		Ok(self.files.replace(file))
	}

	pub fn find_file(&self, file_name: &FileName, dir_name: AsciiPrintingChar) -> Option<&File<'d>> {
		self.files.get(&super::file::Key::new(file_name.clone(), dir_name))
	}

	pub fn remove_file(&mut self, file_name: &FileName, dir_name: AsciiPrintingChar) -> Option<File<'d>> {
		self.files.take(&super::file::Key::new(file_name.clone(), dir_name))
	}

	pub fn to_image(&self, target: &mut dyn io::Write) -> Result<u16, DFSError> {
		use std::ops::Range;
		// first, determine the ordering of files in the disc image
		// then their sector spans, to ensure we have enough space

		use std::num::NonZeroU16;
		struct BuildData<'f, 'd> {
			file: &'f File<'d>,
			start_sector: NonZeroU16,
			sector_count: u16,
		}

		let end_sector;
		let file_indexes = {
			let mut start_sector = NonZeroU16::new(2).unwrap();
			let mut v = self.files.iter().map(|file| Ok(BuildData {
				file,
				start_sector, // to be assigned after sort
				sector_count: match file.content().len() {
					yes if yes <= 0x3ffff => yes.sectors() as u16,
					no => return Err(DFSError::InputTooLarge(no))
				},
			})).collect::<Result<ArrayVec<_, { MAX_FILES as usize }>, _>>()?;
			v.sort_unstable_by_key(|b: &BuildData| b.file.key().clone());

			for data in &mut v {
				data.start_sector = start_sector;
				start_sector = match
				// must not overflow when added to existing sector ptr
				start_sector.get().checked_add(data.sector_count)
				// and must also be non-zero (guaranteed)
				.and_then(NonZeroU16::new) {
					Some(s) => s,
					None => return Err(DFSError::InputTooLarge(0x1_0000)),
				};
			}
			end_sector = start_sector.get();
			v
		};

		if end_sector > MAX_SECTORS {
			return Err(DFSError::InputTooLarge(end_sector as usize));
		}

		let mut sectors = 2u16;
		let mut buf = [0u8; 256];
		let mut write_buf = |buf: &mut [u8; 256], sectors: &mut u16|
		-> Result<(), DFSError> {
			target.write_all(&buf[..])?;
			*buf = [0u8; 256];
			// we only call `write_buf` for first two sectors; it *will not* wrap
			*sectors = sectors.wrapping_add(1);
			Ok(())
		};

		fn buf_for_entry(idx: usize) -> Range<usize> {
			(idx+1)*8 .. (idx+2)*8
		}

		// sector 0: start of disc name, file names
		buf[..8].copy_space_padded(self.name().up_to(8));

		for (i, data) in file_indexes.iter().enumerate() {
			// transform i into offset
			let dst = &mut buf[buf_for_entry(i)];

			// copy file name
			dst[..7].copy_space_padded(data.file.key().name
				.as_ascii_str().as_bytes());
			dst[7] = data.file.key().dir.as_byte();
		}

		write_buf(&mut buf, &mut sectors)?;

		// sector 1: FS metadata mop-up, file entries
		buf[..4].copy_space_padded(self.name().from_up_to(8..12));
		buf[4] = self.cycle().into_u8();
		buf[5] = (self.files.len() as u8).wrapping_mul(8); // won't wrap
		buf[6] = /* b4,5 = boot option  */ (self.boot_option as u8) << 4
		       | /* b0,1 = sectors b8,9 */ ((sectors & 0x300) >> 8) as u8;
		buf[7] = (end_sector & 255) as u8;

		for (i, data) in file_indexes.iter().enumerate() {
			let load  = data.file.load_addr().to_le_bytes();
			let exec  = data.file.exec_addr().to_le_bytes();
			let len   = (data.file.content().len() as u32).to_le_bytes();
			let start = data.start_sector.get().to_le_bytes();
			buf[buf_for_entry(i)].copy_from_slice(&[
				// load low
				load[0], load[1],
				// exec low
				exec[0], exec[1],
				// len low
				len[0], len[1],
				// highs
				((exec[2] & 3) << 6) |
				((len [2] & 3) << 4) |
				((load[2] & 3) << 2) |
				((start[1] & 3) << 0),
				// sector low
				start[0]
			][..]);
		};
		write_buf(&mut buf, &mut sectors)?;

		for data in file_indexes {
			let content = data.file.content();
			target.write_all(content)?;
			match content.len() & 0xff {
				0 => {},
				n =>
					// write_buf is empty
					target.write_all(&buf[n..])?
			};
		}

		Ok(end_sector)
	}
}

pub struct Files<'a, 'd>(::std::collections::hash_set::Iter<'a, File<'d>>);

impl<'a, 'd> Iterator for Files<'a, 'd> {
	type Item = &'a File<'d>;

	fn next(&mut self) -> Option<Self::Item> {
		self.0.next()
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
		// Second half: various addresses
		let offset2 = ((i*8) as usize) + 0x108;

		// Set dir, locked
		let (dir, locked) = {
			let offset = offset1 + 7;
			let raw = src[offset];

			let dir = AsciiPrintingChar::from(raw & 0x7f)
				.map_err(|_| DFSError::InvalidDiscData(offset))?;

			(dir, raw > 0x7f)
		};

		let name = {
			let name_buf = &src[offset1 .. (offset1 + 7)];
			let name_len = name_buf.iter().take_while(|&&b| b > b' ').count();
			FileName::try_from(&name_buf[..name_len]).map_err(|e| {
				let str_pos = e.position();
				DFSError::InvalidDiscData(offset1 + str_pos)
			})?
		};

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
		let file = File::new(name, dir, load_addr, exec_addr, locked,
			Cow::Borrowed(file_contents));

		if files.contains(&file) {
			return Err(DFSError::DuplicateFileName(
				format!("{}.{}", dir, file.name())
				));
		}

		files.insert(file);
	}

	Ok(files)
}

#[cfg(test)]
mod test {

	use crate::dfs;
	use crate::support::*;

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
		assert!(target.is_ok(), "{:?}", target.unwrap_err());
		let target = target.unwrap();

		// Check cycle count
		assert_eq!(BCD::from_hex(0x11).unwrap(), target.cycle());

		for f in target.files() {
			println!("Found file {:?}", f);
		}

		// Start picking files apart
		let check = |dir: u8, name: &str, load: u32, exec: u32, len: usize, byte: u8| {
			println!("Checking {}.{}...", dir, name);
			let file = target.files().find(|&f| {
					f.dir().as_byte() == dir
				}).unwrap_or_else(|| panic!("No file found in dir '{}'", dir));
			assert_eq!(name, file.name());
			assert_eq!(load, file.load_addr());
			assert_eq!(exec, file.exec_addr());
			assert_eq!(len, file.content().len());
			assert!(file.content().iter().all(|&n| n == byte));
		};

		check(b'$', "Small" , 0x1234, 0x5678, 12, 0x31);
		check(b'A', "Single", 0x8765, 0x4321, 256, 0x32);
		check(b'B', "Double", 0x0111, 0x0eee, 257, 0x33);

		assert_eq!(target.files().find(|&f| {
			f.dir().as_byte() == b'C'
		}), None);
	}

	#[test]
	fn disc_name() {
		let test_name = b"DiscName?!";
		let buf = disc_buf_with_name(test_name);

		let target = dfs::Disc::from_bytes(&buf);
		assert!(target.is_ok(), "returned error {:?}", target.unwrap_err());

		let target = target.unwrap();
		assert_eq!(test_name, target.name().as_bytes());
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
		assert_eq!(target.unwrap().name(), disc_name.as_str());

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
			assert_eq!(*boot_type, target.boot_option());
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