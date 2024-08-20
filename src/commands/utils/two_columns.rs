//! These functions have been copy-pasted from internal `poise` code.

use std::fmt::Write as _;

/// Convenience function to align descriptions behind commands
pub struct TwoColumnList(Vec<(String, Option<String>)>);

impl TwoColumnList {
	/// Creates a new [`TwoColumnList`]
	pub fn new() -> Self {
		Self(Vec::new())
	}

	/// Add a line that needs the padding between the columns
	pub fn push_two_colums(&mut self, command: String, description: String) {
		self.0.push((command, Some(description)));
	}

	/// Add a line that doesn't influence the first columns's width
	pub fn push_heading(&mut self, category: &str) {
		if !self.0.is_empty() {
			self.0.push(("".to_string(), None));
		}
		let mut category = category.to_string();
		category += ":";
		self.0.push((category, None));
	}

	/// Convert the list into a string with aligned descriptions
	pub fn into_string(self) -> String {
		let longest_command = self
			.0
			.iter()
			.filter_map(|(command, description)| {
				if description.is_some() {
					Some(command.len())
				} else {
					None
				}
			})
			.max()
			.unwrap_or(0);
		let mut text = String::new();
		for (command, description) in self.0 {
			if let Some(description) = description {
				let padding = " ".repeat(longest_command - command.len() + 3);
				writeln!(text, "{}{}{}", command, padding, description).unwrap();
			} else {
				writeln!(text, "{}", command).unwrap();
			}
		}
		text
	}
}
