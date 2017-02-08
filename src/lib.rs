// dfsdisc -- Library for accessing Acorn DFS-format disc images

extern crate core;

pub mod support;
pub mod dfs;

#[cfg(test)]
mod tests {

	use std::fmt;
	use std::ptr;

	use dfs;
	use support;

	#[test]
	fn disc_name() {

		let test_name = b"DiscName?!";
		let buf = disc_buf_with_name(test_name);

		let target = dfs::Disc::from_bytes(&buf);
		assert!(target.is_ok(), format!("returned error {:?}", target.unwrap_err()));

		let target = target.unwrap().into_inner();
		assert_eq!(test_name, target.disc_name.as_bytes());
	}

	#[test]
	fn disc_name_top_bits_set() {
		let disc_name = b"DiscName";

		for i in 0..8 {
			let mut buf = [0u8; 8];
			support::inject(&mut buf, disc_name).unwrap();
			buf[i] |= 0x80; // set a high bit

			let disc_bytes = disc_buf_with_name(&buf);

			let target = dfs::Disc::from_bytes(&disc_bytes);
			assert!(target.is_err());
			let target = target.unwrap_err();
			assert!(match target {
				dfs::DFSError::InvalidDiscData(at_point) => {
					assert_eq!(i, at_point);
					true
				},
				_ => false
			});
		}
	}

	fn disc_buf_with_name(name: &[u8]) -> [u8 ; dfs::SECTOR_SIZE * 2] {
		let mut buf = [0u8 ; dfs::SECTOR_SIZE * 2];
		support::inject(&mut buf, &name[..8]);
		support::inject(&mut buf[0x100..], &name[8..]);

		buf
	}
}
