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
		use support;

		let mut buf = [0u8 ; dfs::SECTOR_SIZE * 2];
		let test_name = b"DiscName?!";
		support::inject(&mut buf, &test_name[..8]);
		support::inject(&mut buf[0x100..], &test_name[8..]);

		let target = dfs::Disc::from_bytes(&buf);
		assert!(target.is_ok(), format!("returned error {:?}", target.unwrap_err()));

		let target = target.unwrap().into_inner();
		assert_eq!(test_name, target.disc_name.as_bytes());
	}
}
