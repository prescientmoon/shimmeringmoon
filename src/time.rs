// TODO: disable based off env var / feature / idk
#[macro_export]
macro_rules! timed {
	($label:expr, $code:block) => {{
		use std::time::Instant;
		let start = Instant::now();
		let result = { $code }; // Execute the code block
		let duration = start.elapsed();
		println!("📊 {}: {:?}", $label, duration);
		result
	}};
}
