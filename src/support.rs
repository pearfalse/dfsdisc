//! This module contains various support types used by the DFS parser. Most
//! of it is to help validate that bytes from disc images really do contain
//! valid values for what they intend.

use core::ops::Deref;
use std::fmt::{Formatter, Display, Debug, Result as FormatterResult};

/// Overwrites one range of values in a slice with another by copying `src`
/// into `dst`.
///
/// Functionally, this is identical to C's `memcpy`, but with more detection of
/// invalid preconditions. It is designed for byte slices, but will work for
/// any sized intrinsic type.
///
/// # Safety
/// This function uses [`std::mem::copy_nonoverlapping`]
/// (https://doc.rust-lang.org/std/ptr/fn.copy_nonoverlapping.html)
/// under the hood, so its rules about safety also apply here. This function is safe
/// for intrinsic types (e.g. `u8`, `bool`).
///
/// # Errors
/// * `DestinationTooSmall(usize)`: The destination slice doesn't have space to
/// hold all elements from the source. The attached `usize` indicates how many
/// elements short `dst` was.
/// * `SlicesOverlap`: If `dst` and `src` are slices overlapping the same region
/// of memory.
///
/// # Examples
///
/// To change the start of a long byte buffer:
///
/// ```
/// use dfsdisc::support;
///
/// let mut buffer = [0u8; 0x2000];
/// support::inject(&mut buffer, b"NewHeader").unwrap();
/// assert_eq!(b"NewHeader", &buffer[0..9]);
/// ```
///
/// Or, to add values further in:
///
/// ```
/// use dfsdisc::support;
/// use std::iter;
///
/// let mut buffer = [0u16; 250];
/// let trailer = [0xffffu16; 10];
/// support::inject(&mut buffer[240..], &trailer).unwrap();
/// assert!(buffer[240..].iter().all(|&x| x == 0xffff));
/// ```
///
pub fn inject<T>(dst: &mut [T], src: &[T])
-> Result<(), InjectError> where T : Copy + Sized {
	let src_len = src.len();
	if src_len == 0 {
		return Ok(());
	}

	let space: usize = dst.len();
	if src_len > space {
		return Err(InjectError::DestinationTooSmall(src_len - space));
	}

	if slices_overlap(dst, src) {
		return Err(InjectError::SlicesOverlap);
	}

	unsafe {
		use std::ptr;
		let src_p = &src[0] as *const T;
		let dst_p = &mut dst[0] as *mut T;

		ptr::copy_nonoverlapping(src_p, dst_p, src.len());
	}

	Ok(())
}

#[derive(Debug, PartialEq, Eq)]
/// Reasons why `[inject](./fn.inject.html)` may fail.
pub enum InjectError {
	DestinationTooSmall(usize),
	SlicesOverlap,
}

/// Checks whether two slices overlap in memory, such that
/// at least one element shows up in both.
pub fn slices_overlap<T>(a: &[T], b: &[T]) -> bool
where T : Sized {
	let a_len = a.len() as isize;
	let b_len = b.len() as isize;
	if a_len == 0 || b_len == 0 {
		return false;
	}

	// Slices too large to compare for overlapping; very unlikely
	if a_len < 0 || b_len < 0 {
		panic!("slices_overlap failure: one slice is too large (0x{:x} vs 0x{:x}", a.len(), b.len());
	}

	unsafe {
		let a1 = a.get_unchecked(0) as *const T;
		let a2 = a1.offset(a_len);
		let b1 = b.get_unchecked(0) as *const T;
		let b2 = b1.offset(b_len);

		return (a2 > b1) && (b2 > a1);
	}
}

/// Converts a 2-byte slice into a `u16`, assuming a little-endian word layout.
///
/// # Panics
/// The slice must have a length of 2, otherwise this function will panic.
pub fn u16_from_le(src: &[u8]) -> u16 {
	if src.len() != 2 {
		panic!("u16_from_le called with invalid slice length; should be 2, is {}", src.len());
	}
	unsafe {
		((*src.get_unchecked(1) as u16) << 8) | (*src.get_unchecked(0) as u16)
	}
}


#[derive(Clone, Copy, Eq, Debug)]
/// Container for a binary-coded decimal byte.
pub struct BCD {
	value: u8
}

/// Reasons why constructing a [`BCD`] may fail.
///
/// [`BCD`]: struct.BCD.html
#[derive(Debug, PartialEq, Eq)]
pub enum BCDError {
	/// The given integer value was over 99.
	IntValueTooLarge,
	/// The given hex value was not valid BCD.
	InvalidHexValue,
}

impl BCD {
	/// Constructs a `BCD` from a decimal value.
	///
	/// # Errors
	/// Will return a [`BCDError`] if the supplied decimal value was out of
	/// range.
	///
	/// [`BCDError`]: enum.BCDError.html
	pub fn from_u8(src: u8) -> Result<BCD, BCDError> {
		match src {
			x if x <= 99 => {
				Ok(BCD {
					value: ((src / 10) << 4) + (src % 10)
				})
			},
			_ => Err(BCDError::IntValueTooLarge)
		}
	}

	/// Converts a `BCD` back into its decimal value.
	pub fn into_u8(self) -> u8 {
		(self.value >> 4) + (self.value & 15)
	}

	/// Constructs a `BCD` from a pre-encoded BCD representation.
	///
	/// # Errors
	/// Will return a [`BCDError`] if the supplied byte is not valid for BCD.
	///
	/// [`BCDError`]: enum.BCDError.html
	pub fn from_hex(src: u8) -> Result<BCD, BCDError> {
		if ((src & 0xf0) >= 0xa0) || ((src & 0x0f) >= 0x0a) {
			Err(BCDError::InvalidHexValue)
		}
		else {
			Ok(BCD {value: src})
		}
	}
}

impl PartialEq for BCD {
	fn eq(&self, other: &Self) -> bool {
		self.value == other.value
	}
}

/// A `char` wrapper guaranteed to only contain ASCII characters.
///
/// This type dereferences to `char`.
#[derive(PartialEq, Eq, Clone, Hash)]
pub struct AsciiChar {
	value: char,
}

impl AsciiChar {
	/// Constructs an `AsciiChar` from a byte.
	///
	/// # Errors
	/// Will fail if the byte is `0x80` or greater.
	pub fn from_u8(src: u8) -> Result<AsciiChar, ()> {
		match src {
			x if x < 0x80 => Ok(AsciiChar {value: x as char}),
			_ => Err(())
		}
	}
}

impl From<AsciiChar> for u8 {
	fn from(src: AsciiChar) -> Self {
		src.value as u8
	}
}

// Documentation suppressed to prevent all of char's methods leaking in
#[doc(hidden)]
impl Deref for AsciiChar {
	type Target = char;
	fn deref(&self) -> &char {
		&self.value
	}
}

/// Prints the `char` if it is a printable character, otherwise
/// prints a C-style escaped variant.
impl Display for AsciiChar {
	fn fmt(&self, f: &mut Formatter) -> FormatterResult {
		if self.value < '\x20' {
			write!(f, "'\\x{:02x}'", self.value as u8)
		} else {
			write!(f, "{}", self.value)
		}
	}
}

impl Debug for AsciiChar {
	fn fmt(&self, f: &mut Formatter) -> FormatterResult {
		write!(f, "<AsciiChar 0x{:2x}/{}", self.value as u8, self)
	}
}

/// A `char` wrapper guaranteed to only contain printable ASCII characters.
///
/// This type dereferences to `char`.
#[derive(PartialEq, Eq, Clone, Hash)]
pub struct AsciiPrintingChar {
	value: char,
}

impl AsciiPrintingChar {
	/// Constructs an `AsciiPrintingCharChar` from a byte.
	///
	/// # Errors
	/// Will fail if the byte does not map to a printing ASCII character.
	pub fn from_u8(src: u8) -> Result<AsciiPrintingChar, ()> {
		match src {
			x if x >= 0x20 && x < 0x7f => Ok(AsciiPrintingChar {value: x as char}),
			_ => Err(())
		}
	}
}

impl From<AsciiPrintingChar> for u8 {
	fn from(src: AsciiPrintingChar) -> Self {
		src.value as u8
	}
}

// Documentation suppressed to prevent all of char's methods leaking in
#[doc(hidden)]
impl Deref for AsciiPrintingChar {
	type Target = char;
	fn deref(&self) -> &char {
		&self.value
	}
}

/// Prints the underlying `char` directly.
impl Display for AsciiPrintingChar {
	fn fmt(&self, f: &mut Formatter) -> FormatterResult {
		write!(f, "{}", self.value)
	}
}

impl Debug for AsciiPrintingChar {
	fn fmt(&self, f: &mut Formatter) -> FormatterResult {
		write!(f, "<AsciiPrintingChar 0x{:2x}/{}>", self.value as u8, self.value)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_slices_overlap() {
		use std::ops::Range;

		let src = [0u8; 9];
		let check = |a: Range<usize>, b: Range<usize>, expect: bool| {
			let op = |a: &Range<usize>, b: &Range<usize>, expect: bool| {
				let result = slices_overlap(&src[a.clone()], &src[b.clone()]);
				assert_eq!(expect, result, "failed at {:?} vs {:?}", a, b);
			};
			op(&a, &b, expect);
			op(&b, &a, expect);
		};

		check(0..3, 6..9, false); // completely separate
		check(0..5, 5..9, false); // touching
		check(0..5, 4..9, true);  // just overlapping
		check(0..6, 3..9, true);  // cleanly overlapping
		check(0..9, 3..6, true);  // one completely encloses the other
		check(0..9, 6..9, true);  // one edge overlaps
	}

	#[test]
	fn test_inject() {
		use std::slice;
		use std::cell::UnsafeCell;

		// Normal success case
		{
			let mut buf = [0u8; 10];
			let src = b"DATA_SRC";

			let result = inject(&mut buf, src);
			assert!(result.is_ok());
			assert_eq!(b"DATA_SRC\x00\x00", &buf);
		}

		// Destination too small
		{
			let mut buf = [0u8; 1];
			let src = b"FOUR";

			let result = inject(&mut buf, src);
			assert!(result.is_err());
			let result = result.unwrap_err();
			assert_eq!(InjectError::DestinationTooSmall(3), result);
		}

		// Slices overlap
		{
			const ARR_SIZE: usize = 4;
			let buf = UnsafeCell::new([0u8; ARR_SIZE]);
			let mut dst = unsafe {
				slice::from_raw_parts_mut((*buf.get()).get_unchecked_mut(0) as *mut u8, ARR_SIZE)
			};
			let src = unsafe {
				slice::from_raw_parts((*buf.get()).get_unchecked(0) as *const u8, 2)
			};

			let result = inject(dst, src);
			assert!(result.is_err());
			let result = result.unwrap_err();
			assert_eq!(InjectError::SlicesOverlap, result);
		}
	}

	#[test]
	fn bcd_from_u8_success() {
		let inputs = [5u8, 9u8, 10u8, 25u8, 99u8];
		let outputs = [
			BCD {value: 0x05u8},
			BCD {value: 0x09u8},
			BCD {value: 0x10u8},
			BCD {value: 0x25u8},
			BCD {value: 0x99u8},
		];
		for (input, output) in inputs.iter().zip(outputs.iter()) {
			let result = BCD::from_u8(*input);
			assert!(result.is_ok());
			let result = result.unwrap();
			assert_eq!(result, *output);
		}
	}

	#[test]
	fn bcd_from_u8_failure() {
		let inputs = [100u8, 255u8];

		for input in inputs.iter() {
			let result = BCD::from_u8(*input);
			assert!(result.is_err());
			assert_eq!(BCDError::IntValueTooLarge, result.unwrap_err());
		}
	}

	#[test]
	fn bcd_from_hex_success() {
		let op = |input, output| {
			let result = BCD::from_hex(input);
			assert!(result.is_ok());
			let result = result.unwrap();
			assert_eq!(result.value, output);
		};

		op(0x58u8, 0x58);
		op(0x09u8, 0x09);
		op(0x70u8, 0x70);
	}

	#[test]
	fn bcd_from_hex_failure() {
		let op = |input| {
			let result = BCD::from_hex(input);
			assert!(result.is_err());
			assert_eq!(BCDError::InvalidHexValue, result.unwrap_err());
		};

		op(0x0a);
		op(0xa0);
		op(255);
	}

	#[test]
	fn u16_from_le_success() {
		let op = |input: [u8; 2], output: u16| {
			let result = u16_from_le(&input);

			assert_eq!(output, result);
		};

		op([0, 0], 0);
		op([255, 255], 65535);
		op([0x55, 0xaa], 0xaa55);
	}

	#[test]
	fn u16_from_le_failure() {
		use std::panic;

		let op = |input: &[u8]| {
			let caught_panic = panic::catch_unwind(|| { u16_from_le(input) });
			assert!(caught_panic.is_err());
		};

		let data = [77u8];
		op(&data);

		let data = [5, 5, 5];
		op(&data);

		let data = [];
		op(&data);

	}

	#[test]
	fn ascii_char() {
		let check_success = |input: u8| {
			let ch = AsciiChar::from_u8(input);
			assert!(ch.is_ok());
			let ch = ch.unwrap();
			assert_eq!(input, ch.value as u8);
		};

		for i in 0..127 {
			check_success(i);
		}

		let check_failure = |input: u8| {
			let ch = AsciiChar::from_u8(input);
			assert!(ch.is_err());
		};

		for i in 128..256 {
			check_failure(i as u8);
		}
	}

	#[test]
	fn ascii_printing_char() {
		let check_success = |input: u8| {
			let ch = AsciiPrintingChar::from_u8(input);
			assert!(ch.is_ok());
			let ch = ch.unwrap();
			assert_eq!(input, ch.value as u8);
		};

		for i in 32..127 {
			check_success(i);
		}

		let check_failure = |input: u8| {
			let ch = AsciiPrintingChar::from_u8(input);
			assert!(ch.is_err());
		};

		for i in (0..32).chain(127..256) {
			check_failure(i as u8);
		}
	}

	#[test]
	fn format_ascii_char() {
		let op = |input: u8, output: &str| {
			let ch = AsciiChar::from_u8(input).unwrap();
			let result = format!("{}", ch);
			assert_eq!(output, result.as_str());
		};

		op(0x41, "A");
		op(0x33, "3");
		op(0x7f, "\x7f");
		op(0x00, "'\\x00'");
	}

	#[test]
	fn format_ascii_printing_char() {
		let op = |input: u8, output: &str| {
			let ch = AsciiPrintingChar::from_u8(input).unwrap();
			let result = format!("{}", ch);
			assert_eq!(output, result.as_str());
		};

		op(0x41, "A");
		op(0x39, "9");
	}
}
