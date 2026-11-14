use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
pub enum Location {
	Kitchen,
	Forge,
	Alchemy,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Recipe {
	pub name: String,
	pub location: Location,
	/// Number of cards played in one craft.
	pub cards: usize,
	/// Score needed for each star, in ascending order.
	pub stars: Vec<i64>,
}

impl Recipe {
	/// Number of stars the given score earns.
	pub fn stars(&self, score: i64) -> usize {
		self.stars.partition_point(|&threshold| threshold <= score)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn star_thresholds() {
		let recipe = Recipe {
			name: "Test".into(),
			location: Location::Kitchen,
			cards: 5,
			stars: vec![10, 20, 30],
		};
		assert_eq!(recipe.stars(9), 0);
		assert_eq!(recipe.stars(10), 1);
		assert_eq!(recipe.stars(29), 2);
		assert_eq!(recipe.stars(30), 3);
		assert_eq!(recipe.stars(1000), 3);
	}
}
