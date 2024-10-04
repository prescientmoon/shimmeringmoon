use num::{Rational32, ToPrimitive};

pub type Rating = Rational32;

/// Saves a rating rational as an integer where it's multiplied by 100.
#[inline]
pub fn rating_as_fixed(rating: Rating) -> i32 {
	(rating * Rational32::from_integer(100))
		.round()
		.to_integer()
}

/// Saves a rating rational as a float with precision 2.
#[inline]
pub fn rating_as_float(rating: Rating) -> f32 {
	let hundred = Rational32::from_integer(100);
	let rounded = (rating * hundred).round();

	(rounded / hundred).to_f32().unwrap()
}

/// The pseudo-inverse of `rating_as_fixed`.
#[inline]
pub fn rating_from_fixed(fixed: i32) -> Rating {
	Rating::new(fixed, 100)
}
