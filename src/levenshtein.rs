//! Modified version of https://docs.rs/edit-distance/latest/src/edit_distance/lib.rs.html#1-76
//! The primary modification is providing a no-allocation variant
//! for efficient consecutive calls.

/// Similar to `edit_distance`, but takes in a preallocated vec so consecutive calls are efficient.
pub fn edit_distance_with(a: &str, b: &str, cur: &mut Vec<usize>) -> usize {
	let len_a = a.chars().count();
	let len_b = b.chars().count();
	if len_a < len_b {
		return edit_distance_with(b, a, cur);
	}

	// handle special case of 0 length
	if len_a == 0 {
		return len_b;
	} else if len_b == 0 {
		return len_a;
	}

	let len_b = len_b + 1;

	let mut pre;
	let mut tmp;

	cur.clear();
	cur.resize(len_b, 0);

	// initialize string b
	for i in 1..len_b {
		cur[i] = i;
	}

	// calculate edit distance
	for (i, ca) in a.chars().enumerate() {
		// get first column for this row
		pre = cur[0];
		cur[0] = i + 1;
		for (j, cb) in b.chars().enumerate() {
			tmp = cur[j + 1];
			cur[j + 1] = std::cmp::min(
				// deletion
				tmp + 1,
				std::cmp::min(
					// insertion
					cur[j] + 1,
					// match or substitution
					pre + if ca == cb { 0 } else { 1 },
				),
			);
			pre = tmp;
		}
	}
	cur[len_b - 1]
}

/// Returns the edit distance between strings `a` and `b`.
///
/// The runtime complexity is `O(m*n)`, where `m` and `n` are the
/// strings' lengths.
#[inline]
pub fn edit_distance(a: &str, b: &str) -> usize {
	edit_distance_with(a, b, &mut Vec::new())
}
