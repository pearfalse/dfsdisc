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
}
