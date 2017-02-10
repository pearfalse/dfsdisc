// Support stuff

pub fn inject<T>(dst: &mut [T], src: &[T])
-> Result<(), usize> where T : Copy + Sized {
	let src_len = src.len();
	if src_len == 0 {
		return Ok(());
	}

	let space: usize = dst.len();
	if src_len > space {
		return Err(src_len - space);
	}

	unsafe {
		use std::ptr;
		let src_p = &src[0] as *const T;
		let dst_p = &mut dst[0] as *mut T;
		ptr::copy_nonoverlapping(src_p, dst_p, src.len());
	}

	Ok(())
}


#[derive(Clone, Copy, Eq, Debug)]
pub struct BCD {
	value: u8
}

#[derive(Debug, PartialEq, Eq)]
pub enum BCDError {
	IntValueTooLarge,
}

impl BCD {
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

	pub fn into_u8(self) -> u8 {
		(self.value >> 4) + (self.value & 15)
	}
}

impl PartialEq for BCD {
	fn eq(&self, other: &Self) -> bool {
		self.value == other.value
	}
}


#[derive(PartialEq, Eq, Debug, Clone, Hash)]
pub struct AsciiChar {
	value: u8,
}

impl AsciiChar {
	pub fn from_u8(src: u8) -> Result<AsciiChar, ()> {
		match src {
			x if x < 0x80 => Ok(AsciiChar {value: x}),
			_ => Err(())
		}
	}
}

impl From<AsciiChar> for u8 {
	fn from(src: AsciiChar) -> Self {
		src.value
	}
}

impl From<AsciiChar> for char {
	fn from(src: AsciiChar) -> Self {
		src.value as char
	}
}


#[derive(PartialEq, Eq, Debug, Clone, Hash)]
pub struct AsciiPrintingChar {
	value: u8,
}

impl AsciiPrintingChar {
	pub fn from_u8(src: u8) -> Result<AsciiPrintingChar, ()> {
		match src {
			x if x >= 0x20 && x < 0x80 => Ok(AsciiPrintingChar {value: x}),
			_ => Err(())
		}
	}
}

impl From<AsciiPrintingChar> for u8 {
	fn from(src: AsciiPrintingChar) -> Self {
		src.value
	}
}

impl From<AsciiPrintingChar> for char {
	fn from(src: AsciiPrintingChar) -> Self {
		src.value as char
	}
}


#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn inject_success() {
		let mut buf = [0u8; 10];
		let src = b"DATA_SRC";

		let result = inject(&mut buf, src);
		assert!(result.is_ok());
		assert_eq!(b"DATA_SRC\x00\x00", &buf);
	}

	#[test]
	fn inject_fail() {
		let mut buf = [0u8; 1];
		let src = b"FOUR";

		let result = inject(&mut buf, src);
		assert!(result.is_err());
		let result = result.unwrap_err();
		assert_eq!(3, result);
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
			assert_eq!(result.unwrap_err(), BCDError::IntValueTooLarge);
		}
	}
}
