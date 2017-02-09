// dfsdisc -- Library for accessing Acorn DFS-format disc images

extern crate core;

pub mod support;
pub mod dfs;

#[cfg(test)]
mod tests {

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
			let target = target.unwrap().into_inner();
			assert_eq!(*boot_type, target.boot_option);
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

	#[test]
	fn test_everything() {
		// TODO: disc cycle, sector count,
	}

	fn disc_buf_with_name(name: &[u8]) -> [u8 ; dfs::SECTOR_SIZE * 2] {
		let mut buf = [0u8 ; dfs::SECTOR_SIZE * 2];
		support::inject(&mut buf, &name[..8]).unwrap();
		support::inject(&mut buf[0x100..], &name[8..]).unwrap();
		buf[0x107] = 2; // sector count
		buf
	}
}
