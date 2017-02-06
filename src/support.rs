// Support stuff

pub mod support {
	pub fn inject<T>(dst: &mut [T], offset: usize, src: &[T])
	-> Result<(), usize> where T : Copy + Sized {
		let src_len = src.len();
		if src_len == 0 {
			return Ok(());
		}

		let space: usize = dst.len() - offset;
		if src_len > space {
			return Err(src_len - space);
		}

		unsafe {
			use std::ptr;
			let src_p = &src[0] as *const T;
			let dst_p = &mut dst[0] as *mut T;
			ptr::copy_nonoverlapping(src_p, dst_p.offset(offset as isize), src.len());
		}

		Ok(())
	}

	#[cfg(test)]
	mod tests {
		use super::*;

		fn success_body(offset: usize, expected: &[u8]) {
			let mut buf = [0u8; 10];
			let src = b"DATA_SRC";

			let result = inject(&mut buf, offset, src);
			assert!(result.is_ok());
			assert_eq!(expected, &buf);
		}

		#[test]
		fn inject_positive_budget() {
			success_body(0, b"DATA_SRC\x00\x00");
		}

		#[test]
		fn inject_positive_budget_offset() {
			success_body(1, b"\x00DATA_SRC\x00");
		}

		#[test]
		fn inject_zero_budget() {
			success_body(2, b"\x00\x00DATA_SRC")
		}

		#[test]
		fn inject_negative_budget() {
			let mut buf = [0u8; 1];
			let src = b"FOUR";

			let result = inject(&mut buf, 0, src);
			assert!(result.is_err());
			let result = result.unwrap_err();
			assert_eq!(3, result);
		}
	}
}
