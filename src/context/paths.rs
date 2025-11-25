//! This module provides helpers for working with environment
//! variables and paths, together with a struct
//! that keeps track of all the runtime-relevant paths.

use anyhow::Context;
use std::{path::Path, path::PathBuf, str::FromStr};

/// Wrapper around [std::env::var] which adds [anyhow] context around errors.
pub fn get_var(name: &str) -> anyhow::Result<String> {
	std::env::var(name).with_context(|| format!("Missing ${name} environment variable"))
}

/// Reads an environment variable containing a directory path,
/// creating the directory if it doesn't exist.
pub fn get_env_dir_path(name: &str, default_to: Option<&str>) -> anyhow::Result<PathBuf> {
	let var = get_var(name);
	let var = match default_to {
		None => var?,
		Some(other) => var.or(get_var(other))?,
	};

	let path = PathBuf::from_str(&var).with_context(|| format!("${name} is not a valid path"))?;

	if !path.exists() {
		std::fs::create_dir_all(&path).with_context(|| format!("Could not create ${name}"))?;
	}

	Ok(path)
}

#[derive(Clone, Debug)]
pub struct ShimmeringPaths {
	/// This directory contains files that are entirely managed
	/// by the runtime of the app, like databases or processed
	/// jacket art.
	data_dir: PathBuf,

	/// This directory contains configuration that should
	/// not be public, like the directory of raw jacket art.
	private_config_dir: PathBuf,

	/// This directory contains logs and other debugging info.
	log_dir: PathBuf,
}

impl ShimmeringPaths {
	/// Gets all the standard paths from the environment,
	/// creating every involved directory in the process.
	pub fn new() -> anyhow::Result<Self> {
		let res = Self {
			data_dir: get_env_dir_path("SHIMMERING_DATA_DIR", Some("STATE_DIRECTORY"))?,
			log_dir: get_env_dir_path("SHIMMERING_LOG_DIR", Some("LOGS_DIRECTORY"))?,
			private_config_dir: get_env_dir_path("SHIMMERING_PRIVATE_CONFIG_DIR", None)?,
		};

		Ok(res)
	}

	pub fn data_dir(&self) -> &PathBuf {
		&self.data_dir
	}

	pub fn log_dir(&self) -> &PathBuf {
		&self.log_dir
	}

	pub fn db_path(&self) -> PathBuf {
		self.data_dir.join("db.sqlite")
	}

	pub fn jackets_path(&self) -> PathBuf {
		self.data_dir.join("jackets")
	}

	pub fn recognition_matrix_path(&self) -> PathBuf {
		self.data_dir.join("recognition_matrix")
	}

	pub fn raw_jackets_path(&self) -> PathBuf {
		self.private_config_dir.join("jackets")
	}

	pub fn songlist_path(&self) -> PathBuf {
		self.private_config_dir.join("songlist.json")
	}

	pub fn cc_data_path(&self) -> PathBuf {
		get_env_dir_path("SHIMMERING_CC_FILE", None).unwrap()
	}
}

/// Ensures an empty directory exists at a given path,
/// creating it if it doesn't, and emptying it's contents if it does.
pub fn create_empty_directory(path: &Path) -> anyhow::Result<()> {
	if path.exists() {
		std::fs::remove_dir_all(path).with_context(|| format!("Could not remove `{path:?}`"))?;
	}

	std::fs::create_dir_all(path).with_context(|| format!("Could not create `{path:?}` dir"))?;
	Ok(())
}
