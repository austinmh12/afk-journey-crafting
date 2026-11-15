use serde::Deserialize;

use crate::score::Score;

/// Names of recipes with special powers.
pub mod powers {
	/// Cut always yields its max points.
	pub const SALTED_RAISIN: &str = "Salted Raisin";
	/// Cut always hits the lowest colour.
	pub const ROASTED_CORN: &str = "Roasted Corn";
	/// +3 to all colours at the end if Heat Control triggered at least 7 times.
	pub const DRIED_MUSHROOM: &str = "Dried Mushroom";
	/// +5 to all colours at the end if the gap between colours is less than 5.
	pub const ODD_SWEET: &str = "Odd Sweet";
	/// At the end, the highest colour is lowered to match the lowest.
	pub const HEALTHY_CANDIED_SKEWER: &str = "Healthy Candied Skewer";
	/// Forge Expert has a 30% chance to trigger 1 extra time.
	pub const COPPER_STEWPOT: &str = "Copper Stewpot";
	/// The first Forge card hits every colour.
	pub const CARVED_BOX: &str = "Carved Box";
	/// +3 to all colours at the end if artisan cards triggered at least 6 times.
	pub const WARM_STONE_ARMOR: &str = "Warm Stone Armor";
	/// Forge Expert has a 30% chance to trigger 1 extra time.
	pub const FIREFANG_SWORD: &str = "Firefang Sword";
	/// The first 3 Forge cards hit every colour.
	pub const FIREPROOF_HELM: &str = "Fireproof Helm";
	/// Forge, Forge Expert, Pyroburst and Soulbind flip again if enhanced by at least 2 other cards.
	pub const PROTECTIVE_PICK_HAMMER: &str = "Protective Pick Hammer";
}

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

	/// Applies this recipe's end-of-production power, if it has one. Runs after the
	/// cards' own end effects (Exact Cook, Bake, Fuse).
	pub fn apply_end_power(&self, score: &mut Score) {
		match self.name.as_str() {
			powers::DRIED_MUSHROOM if score.heat_control_triggers >= 7 => score.add_all(3),
			powers::WARM_STONE_ARMOR if score.artisan_triggers >= 6 => score.add_all(3),
			powers::ODD_SWEET if (score.left - score.right).abs() < 5 => score.add_all(5),
			powers::HEALTHY_CANDIED_SKEWER => {
				let lowest = score.lowest();
				score.left = lowest;
				score.right = lowest;
			}
			_ => {}
		}
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
