use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::card::CardKind;
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
	/// +3 to the highest colour after each card flip.
	pub const CALMING_WARMDUST: &str = "Calming Warmdust";
	/// +3 to the highest colour after each card flip.
	pub const SOOTHING_TONIC: &str = "Soothing Tonic";
	/// +1 to the lowest colour after each card flip.
	pub const WARMDUST: &str = "Warmdust";
	/// +1 to the lowest colour after each card flip.
	pub const ILLUSION_POTION: &str = "Illusion Potion";
	/// Grind triggers 1 extra time.
	pub const WARMING_INCENSE: &str = "Warming Incense";
	/// Ingredients triggers 1 extra time.
	pub const CALMWIND_INCENSE: &str = "Calmwind Incense";
	/// +15 to a random colour at the start of production.
	pub const FIREWARD_RING: &str = "Fireward Ring";
	/// Every effect on the highest colour is applied twice.
	pub const EARTH_ANTIDOTE: &str = "Earth Antidote";
	/// At the end, the lowest colour doubles if the highest is more than 3x the lowest.
	pub const FROST_ANTIDOTE: &str = "Frost Antidote";
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(clap::ValueEnum))]
pub enum Location {
	Kitchen,
	Forge,
	Alchemy,
}

impl Location {
	pub const ALL: [Self; 3] = [Self::Kitchen, Self::Forge, Self::Alchemy];

	pub fn name(self) -> &'static str {
		match self {
			Self::Kitchen => "Kitchen",
			Self::Forge => "Forge",
			Self::Alchemy => "Alchemy",
		}
	}
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

	/// What this recipe's power does, worded as in the game, or `None` if it has
	/// no power the calculator knows of.
	pub fn power(&self) -> Option<&'static str> {
		Some(match self.name.as_str() {
			powers::SALTED_RAISIN => "Cutting always yields max points",
			powers::ROASTED_CORN => "Cut always affects the lowest color",
			powers::DRIED_MUSHROOM => {
				"+3 to all colors if Heat Control triggers at least 7 times at the end of production"
			}
			powers::ODD_SWEET => "+5 to all colors if the final point gap is less than 5",
			powers::HEALTHY_CANDIED_SKEWER => "Lowers the highest color to match the lowest color at the end of production",
			powers::COPPER_STEWPOT | powers::FIREFANG_SWORD => "30% chance for Forge Expert to trigger an extra time",
			powers::CARVED_BOX => "The first Forge card affects all colors",
			powers::WARM_STONE_ARMOR => {
				"+3 to all colors if artisan card effects triggered at least 6 times at the end of production"
			}
			powers::FIREPROOF_HELM => "The first 3 Forge cards affect all colors",
			powers::PROTECTIVE_PICK_HAMMER => "Cards enhanced by at least 2 other cards trigger an extra time",
			powers::CALMING_WARMDUST | powers::SOOTHING_TONIC => "+3 to the highest color for each card flip",
			powers::WARMDUST | powers::ILLUSION_POTION => "+1 to the lowest color for each card flip",
			powers::WARMING_INCENSE => "Grind cards trigger 1 extra time",
			powers::CALMWIND_INCENSE => "Ingredients cards trigger 1 extra time",
			powers::FIREWARD_RING => "+15 to a random color at the start of production",
			powers::EARTH_ANTIDOTE => "Any effects on the highest color are doubled",
			powers::FROST_ANTIDOTE => {
				"Doubles the lowest color at the end of production if the highest color is more than 3x the lowest"
			}
			_ => return None,
		})
	}

	/// The starting score for this recipe, with any start-of-production power applied.
	pub fn start_score<R: Rng + ?Sized>(&self, rng: &mut R) -> Score {
		let mut score = Score::default();
		if self.location == Location::Alchemy {
			// Alchemy colours can never drop below 1
			score.floor = 1;
		}
		match self.name.as_str() {
			powers::FIREWARD_RING => score.add_random(15, rng),
			powers::EARTH_ANTIDOTE => score.highest_repeats = 2,
			_ => {}
		}
		score
	}

	/// How many times a card of `kind` triggers each time it flips.
	pub fn triggers(&self, kind: CardKind) -> u32 {
		match (self.name.as_str(), kind) {
			(powers::WARMING_INCENSE, CardKind::Grind) | (powers::CALMWIND_INCENSE, CardKind::Ingredients) => 2,
			_ => 1,
		}
	}

	/// Applies this recipe's power that runs after every card flip, if it has one.
	pub fn apply_flip_power(&self, score: &mut Score) {
		match self.name.as_str() {
			powers::CALMING_WARMDUST | powers::SOOTHING_TONIC => score.add_highest(3),
			powers::WARMDUST | powers::ILLUSION_POTION => score.add_lowest(1),
			_ => {}
		}
	}

	/// Applies this recipe's end-of-production power, if it has one. Runs after the
	/// cards' own end effects (Exact Cook, Bake, Fuse).
	pub fn apply_end_power(&self, score: &mut Score) {
		match self.name.as_str() {
			powers::FROST_ANTIDOTE if score.left.max(score.right) > 3 * score.lowest() => score.double_lowest(),
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

	#[test]
	fn powers_are_described() {
		let recipes: Vec<Recipe> = ron::from_str(include_str!("../data/recipes.ron")).unwrap();
		let find = |name: &str| recipes.iter().find(|recipe| recipe.name == name).unwrap();
		assert_eq!(find(powers::CARVED_BOX).power(), Some("The first Forge card affects all colors"));
		assert_eq!(find("Round Bread").power(), None);
		// Every power constant names a recipe in the data, so its description can show
		let described = recipes.iter().filter(|recipe| recipe.power().is_some()).count();
		assert_eq!(described, 20);
	}
}
