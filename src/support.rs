//! This module contains various support types used by the DFS parser. Most
//! of it is to help validate that bytes from disc images really do contain
//! valid values for what they intend.

use std::fmt;

use ascii;
use ascii::AsciiChar;

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

#[derive(Debug)]
pub enum AsciiPrintingCharError {
	AsciiConversionError(ascii::ToAsciiCharError),
	NonprintingChar,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct AsciiPrintingChar(AsciiChar);

impl AsciiPrintingChar {
	pub fn from<C: ascii::ToAsciiChar>(src: C)
	-> Result<AsciiPrintingChar, AsciiPrintingCharError> {
		let maybe = ascii::ToAsciiChar::to_ascii_char(src)
			.map_err(AsciiPrintingCharError::AsciiConversionError)?;
		if maybe.is_control() {
			Err(AsciiPrintingCharError::NonprintingChar)
		}
		else {
			Ok(AsciiPrintingChar(maybe))
		}
	}

	pub fn as_byte(&self) -> u8 {
		self.0.as_byte()
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
