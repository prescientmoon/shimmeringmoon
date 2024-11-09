use sha2::{Digest, Sha256};

pub fn hash_files(path: &std::path::Path) -> anyhow::Result<String> {
	fn hash_dir_files_rec(path: &std::path::Path, hasher: &mut Sha256) -> anyhow::Result<()> {
		if path.is_dir() {
			for entry in std::fs::read_dir(path)? {
				let path = entry?.path();
				hash_dir_files_rec(&path, hasher)?;
			}
		} else if path.is_file() {
			let mut file = std::fs::File::open(path)?;
			hasher.update(path.to_str().unwrap().as_bytes());
			std::io::copy(&mut file, hasher)?;
		}

		Ok(())
	}

	let mut hasher = Sha256::default();
	hash_dir_files_rec(path, &mut hasher)?;
	let res = hasher.finalize();
	let string = base16ct::lower::encode_string(&res);
	Ok(string)
}
