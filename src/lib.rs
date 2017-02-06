// dfsdisc -- Library for accessing Acorn DFS-format disc images

extern crate core;

pub mod support;
pub mod dfs;

mod dfsdisc {

	pub use dfs;
	pub use support;

}

#[cfg(test)]
mod tests {

	use std::fmt;
	use std::ptr;

	use dfs;

	#[test]
	fn disc_name() {
		let mut buf = [0u8 ; dfs::SECTOR_SIZE * 2];
		let test_name = b"DiscName?!";
		unsafe {
			let src = test_name as *const u8;
			let dst = &mut buf[0] as *mut u8;
			ptr::copy_nonoverlapping(src, dst, test_name.len());
		}

		let target = dfs::Disc::from_bytes(&buf);
		assert!(target.is_ok(), format!("returned error {:?}", target.unwrap_err()));

		let target = target.unwrap();
		//assert_eq!(&test_name, target.disc_name.bytes());
	}
}
