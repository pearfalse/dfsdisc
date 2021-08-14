//! This module contains various support types used by the DFS parser. Most
//! of it is to help validate that bytes from disc images really do contain
//! valid values for what they intend.

use std::fmt;
use std::ops::Deref;

use ascii;
use ascii::{AsciiChar, AsciiStr};
use arrayvec::ArrayVec;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct SliceMinSizeError;

/// Tries to convert an array slice to a reference to a fixed-size array.
///
/// Unlike `&[T; N] as TryFrom<[T]>`, this trait _will_ succeed if the slice
/// is bigger; only the first N elements will be considered.
pub trait ArrayFromMinSlice<T, const N: usize> {
	/// Attempt the conversion.
	fn as_min_slice(&self) -> Result<&[T; N], SliceMinSizeError>;
}

impl<T, const N: usize> ArrayFromMinSlice<T, N> for [T] {
	fn as_min_slice(&self) -> Result<&[T; N], SliceMinSizeError> {
		match self.len() {
			n if n >= N => unsafe {
				// SAFETY: src.len() ensured to be big enough
				Ok(&*(self.as_ptr() as *const [T; N]))
			},
			_ => return Err(SliceMinSizeError),
		}
	}
}


pub trait CopyFromCommonSliceExt<T> {
	fn copy_from_common_slice(&mut self, src: &[T]);
}

impl<T> CopyFromCommonSliceExt<T> for [T] where T: Copy + Sized {
	fn copy_from_common_slice(&mut self, src: &[T]) {
		let max_size = self.len().min(src.len());
		self[..max_size].copy_from_slice(&src[..max_size])
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
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
// TODO: is this a meaningful disctinction? >99 is also not valid BCD
pub enum BCDError {
	/// The given integer value was over 99.
	IntValueTooLarge,
	/// The given hex value was not valid BCD.
	InvalidHexValue,
}

impl BCD {

	pub const C00: BCD = unsafe { BCD::new_unchecked(0x00) };
	pub const C99: BCD = unsafe { BCD::new_unchecked(0x99) };

	/// Constructs a `BCD` from a decimal value.
	///
	/// # Errors
	/// Will return a [`BCDError`] if the supplied decimal value was out of
	/// range.
	///
	/// [`BCDError`]: enum.BCDError.html
	pub fn try_new(src: u8) -> Result<BCD, BCDError> {
		match src {
			x if x <= 99 => {
				Ok(BCD {
					value: ((src / 10) << 4) + (src % 10)
				})
			},
			_ => Err(BCDError::IntValueTooLarge)
		}
	}

	/// Constructs a `BCD` without checking if it is valid BCD first.
	///
	/// # Safety
	///
	/// `src` must be a valid BCD value.
	pub const unsafe fn new_unchecked(src: u8) -> BCD {
		Self { value: src }
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

#[derive(Debug)]
pub enum AsciiPrintingCharError {
	AsciiConversionError(ascii::ToAsciiCharError),
	NonprintingChar,
	TooManyChars,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct AsciiPrintingChar(AsciiChar);

impl AsciiPrintingChar {
	pub fn from<C: ascii::ToAsciiChar>(src: C)
	-> Result<AsciiPrintingChar, AsciiPrintingCharError> {
		let maybe = ascii::ToAsciiChar::to_ascii_char(src)
			.map_err(AsciiPrintingCharError::AsciiConversionError)?;
		if maybe.as_char().is_control() {
			Err(AsciiPrintingCharError::NonprintingChar)
		}
		else {
			Ok(AsciiPrintingChar(maybe))
		}
	}

	pub const DOLLAR: AsciiPrintingChar = Self(AsciiChar::Dollar);

	pub fn try_from_str(s: &str) -> Result<AsciiPrintingChar, AsciiPrintingCharError> {
		use std::convert::TryFrom;
		let ch = <[u8; 1]>::try_from(s.as_bytes()).map_err(|_| AsciiPrintingCharError::TooManyChars)?[0];
		Self::from(ch)
	}

	pub fn as_byte(&self) -> u8 {
		self.0.as_byte()
	}

	pub fn as_ascii_char(self) -> AsciiChar { self.0 }

	pub fn as_ascii_str(&self) -> &AsciiStr {
		std::slice::from_ref(self).as_ascii_str()
	}
}

impl std::ops::Deref for AsciiPrintingChar {
	type Target = AsciiChar;

	fn deref(&self) -> &Self::Target {
	    &self.0
	}
}

impl fmt::Display for AsciiPrintingChar {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		self.0.fmt(f)
	}
}

impl From<AsciiPrintingChar> for AsciiChar {
	fn from(src: AsciiPrintingChar) -> AsciiChar {
		src.0
	}
}

impl ascii::ToAsciiChar for AsciiPrintingChar {
	unsafe fn to_ascii_char_unchecked(self) -> AsciiChar { self.0 }

	fn to_ascii_char(self) -> Result<AsciiChar, ascii::ToAsciiCharError> { Ok(self.0) }
}

pub type AsciiPrintingStr = [AsciiPrintingChar];

pub trait AsciiPrintingSlice {
	fn try_from_str(src: &str) -> Result<&AsciiPrintingStr, AsciiPrintingCharError>;
	fn as_ascii_str(&self) -> &AsciiStr;
}

impl AsciiPrintingSlice for AsciiPrintingStr {
	fn try_from_str(src: &str) -> Result<&AsciiPrintingStr, AsciiPrintingCharError> {
		for &ch in src.as_bytes().iter() {
			AsciiPrintingChar::from(ch)?;
		}

		Ok(unsafe { &*(src as *const str as *const [AsciiPrintingChar]) })
	}

	fn as_ascii_str(&self) -> &AsciiStr {
		unsafe { &*(self as *const AsciiPrintingStr as *const AsciiStr) }
	}
}


#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub struct AsciiNameError(usize);

impl AsciiNameError {
	pub fn position(&self) -> usize { self.0 }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct AsciiName<const N: usize> {
	store: ArrayVec<AsciiPrintingChar, N>,
}

impl<const N: usize> AsciiName<N> {
	pub fn try_from<C>(src: &[C]) -> Result<Self, AsciiNameError>
	where C: ascii::ToAsciiChar + Copy {
		let mut store = ArrayVec::new();
		for (i, byte) in src.iter().enumerate() {
			let apc = AsciiPrintingChar::from(*byte).map_err(|_| AsciiNameError(i))?;
			store.try_push(apc).map_err(|_| AsciiNameError(i))?;
		}

		Ok(Self { store })
	}

	pub fn empty() -> AsciiName<N> {
		Self { store: ArrayVec::new() }
	}

	pub fn as_ascii_str(&self) -> &AsciiStr {
		(*self.store).as_ascii_str()
	}
}

impl<const N: usize> Deref for AsciiName<N> {
	type Target = [AsciiPrintingChar];

	fn deref(&self) -> &Self::Target { &*self.store }
}

impl<const N: usize> std::fmt::Display for AsciiName<N> {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		(*self.store).as_ascii_str().fmt(f)
	}
}

#[cfg(test)]
mod test_array_from_min_slice {
	use super::*;

	static SRC: [u8; 3] = [1,2,3];

	#[test]
	fn slice_big_enough() {
		let dst = [1u8,2];
		assert_eq!(Ok(&dst), SRC[..2].as_min_slice());
	}

	#[test]
	fn slice_exact_size() {
		assert_eq!(Ok(&SRC), SRC[..].as_min_slice());
	}

	#[test]
	fn slice_too_small() {
		let got: Result<&[u8; 4], _> = SRC[..].as_min_slice();
		assert_eq!(Err(SliceMinSizeError), got);
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn copy_from_common_slice() {
		let full1 = b"01234567";
		let full2 = b"ABCDEFGH";

		let case = |r1: ::std::ops::Range<usize>, r2: ::std::ops::Range<usize>, result: &'static [u8]| {
			let mut buf = [0u8; 8];
			buf.copy_from_slice(full1);
			buf[r1].copy_from_common_slice(&full2[r2]);
			assert_eq!(buf, result);
		};

		case(0..4, 0..4, b"ABCD4567");
		case(2..6, 1..5, b"01BCDE67");
		case(4..8, 0..3, b"0123ABC7");
		case(0..1, 0..8, b"A1234567");
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
		for (input, output) in inputs.iter().copied().zip(outputs.iter().copied()) {
			assert_eq!(Ok(output), BCD::try_new(input));
		}
	}

	#[test]
	fn bcd_from_u8_failure() {
		let inputs = [100u8, 255u8];

		for input in inputs.iter().copied() {
			assert_eq!(Err(BCDError::IntValueTooLarge), BCD::try_new(input));
		}
	}

	#[test]
	fn bcd_from_hex_success() {
		let op = |input, output| assert_eq!(Ok(output), BCD::from_hex(input).map(|bcd| bcd.value));

		op(0x58u8, 0x58);
		op(0x09u8, 0x09);
		op(0x70u8, 0x70);
	}

	#[test]
	fn bcd_from_hex_failure() {
		let op = |input| assert_eq!(Err(BCDError::InvalidHexValue), BCD::from_hex(input));

		op(0x0a);
		op(0xa0);
		op(255);
	}

	#[test]
	fn u16_from_le_success() {
		let op = |input: [u8; 2], output: u16| assert_eq!(output, u16_from_le(&input));

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
	fn ascii_printing_char() {

		for i in 32..127 {
			let ch = super::AsciiPrintingChar::from(i as u8);
			assert!(ch.is_ok());
			let ch = ch.unwrap();
			assert_eq!(i as u8, ch.as_byte());
		}

		for i in (0..32).chain(127..256) {
			let ch = super::AsciiPrintingChar::from(i as u8);
			assert!(ch.is_err());
		}
	}
}
