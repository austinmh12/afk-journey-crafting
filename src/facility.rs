//! What each facility level allows, loaded from `data/facilities.ron`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::card::CardKind;
use crate::inventory::Owned;
use crate::recipe::{Location, Recipe};

/// The highest facility level in the game.
pub const MAX_LEVEL: u8 = 16;

/// Caps for one location, by facility level.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FacilityTable {
	/// The highest level each card can be at.
	#[serde(default)]
	pub card_levels: BTreeMap<u8, BTreeMap<CardKind, u8>>,
	/// The most copies of each card that can go in one deck.
	#[serde(default)]
	pub card_limits: BTreeMap<u8, BTreeMap<CardKind, usize>>,
	/// The names of the recipes that unlock at each level.
	#[serde(default)]
	pub recipe_unlocks: BTreeMap<u8, Vec<String>>,
}

impl FacilityTable {
	/// The facility level the recipe named `name` unlocks at, if it's listed.
	pub fn recipe_unlock_level(&self, name: &str) -> Option<u8> {
		self.recipe_unlocks
			.iter()
			.find(|(_, names)| names.iter().any(|listed| listed == name))
			.map(|(&level, _)| level)
	}
}

/// Caps for every location.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Facilities(pub BTreeMap<Location, FacilityTable>);

impl Facilities {
	/// What `facility_level` at `location` allows for a card of `kind`.
	pub fn cap(&self, location: Location, facility_level: u8, kind: CardKind) -> Cap {
		let Some(table) = self.0.get(&location) else {
			return Cap::default();
		};
		Cap {
			level: table.card_levels.get(&facility_level).and_then(|cards| cards.get(&kind)).copied(),
			limit: table.card_limits.get(&facility_level).and_then(|cards| cards.get(&kind)).copied(),
		}
	}

	/// The facility level `recipe` unlocks at, if it's listed.
	pub fn recipe_unlock_level(&self, recipe: &Recipe) -> Option<u8> {
		self.0.get(&recipe.location)?.recipe_unlock_level(&recipe.name)
	}

	/// Whether `recipe` can be crafted at `facility_level`. Recipes that
	/// aren't listed count as unlocked.
	pub fn recipe_unlocked(&self, recipe: &Recipe, facility_level: u8) -> bool {
		self.recipe_unlock_level(recipe).is_none_or(|unlock| unlock <= facility_level)
	}
}

/// The most a facility level allows for one card. `None` means there's no data,
/// so there's no cap.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Cap {
	pub level: Option<u8>,
	pub limit: Option<usize>,
}

impl Cap {
	/// Whether the card isn't unlocked yet.
	pub fn locked(self) -> bool {
		self.level == Some(0) || self.limit == Some(0)
	}

	/// `owned`, lowered to fit this cap. A locked card gets a limit of 0.
	pub fn apply(self, owned: Owned) -> Owned {
		if self.locked() {
			return Owned { limit: 0, ..owned };
		}
		Owned {
			level: self.level.map_or(owned.level, |cap| owned.level.min(cap)),
			limit: self.limit.map_or(owned.limit, |cap| owned.limit.min(cap)),
		}
	}

	/// The highest values this cap allows, keeping `owned`'s value where there's no data.
	pub fn maximum(self, owned: Owned) -> Owned {
		if self.locked() {
			return Owned { limit: 0, ..owned };
		}
		Owned {
			level: self.level.unwrap_or(owned.level),
			limit: self.limit.unwrap_or(owned.limit),
		}
	}
}

#[cfg(test)]
pub(crate) mod tests {
	use super::*;

	fn facilities() -> Facilities {
		ron::from_str(include_str!("../data/facilities.ron")).expect("data/facilities.ron should parse")
	}

	#[test]
	fn data_file_covers_every_location() {
		let facilities = facilities();
		for location in Location::ALL {
			let table = &facilities.0[&location];
			assert!(!table.card_levels.is_empty(), "{location:?} has no card levels");
			let kinds = table
				.card_levels
				.values()
				.flat_map(BTreeMap::keys)
				.chain(table.card_limits.values().flat_map(BTreeMap::keys));
			for kind in kinds {
				assert_eq!(kind.location(), location, "{kind:?} is listed under {location:?}");
			}
		}
	}

	#[test]
	fn recipe_unlocks_match_recipes() {
		let facilities = facilities();
		let recipes: Vec<Recipe> = ron::from_str(include_str!("../data/recipes.ron")).unwrap();
		for (location, table) in &facilities.0 {
			let mut seen = std::collections::BTreeSet::new();
			for name in table.recipe_unlocks.values().flatten() {
				let recipe = recipes.iter().find(|recipe| &recipe.name == name);
				assert!(recipe.is_some(), "{name:?} in {location:?}'s recipe_unlocks isn't in recipes.ron");
				assert_eq!(recipe.unwrap().location, *location, "{name:?} is listed under {location:?}");
				assert!(seen.insert(name), "{name:?} is listed more than once under {location:?}");
			}
		}
	}

	#[test]
	fn recipes_unlock_at_their_level() {
		let recipe = |name: &str| Recipe {
			name: name.into(),
			location: Location::Forge,
			cards: 3,
			stars: vec![],
		};
		let facilities = Facilities(BTreeMap::from([(
			Location::Forge,
			FacilityTable {
				recipe_unlocks: BTreeMap::from([(1, vec!["Early".into()]), (5, vec!["Late".into()])]),
				..FacilityTable::default()
			},
		)]));

		assert!(facilities.recipe_unlocked(&recipe("Early"), 1));
		assert!(!facilities.recipe_unlocked(&recipe("Late"), 4));
		assert!(facilities.recipe_unlocked(&recipe("Late"), 5));
		// Not listed, so unlocked
		assert!(facilities.recipe_unlocked(&recipe("Unknown"), 1));
	}

	/// Kitchen caps for tests, independent of data/facilities.ron:
	/// level 5 caps Cut to level 4 with 3 copies and hasn't unlocked Ferment;
	/// level 6 only has card levels. Nothing else is listed.
	pub(crate) fn test_facilities() -> Facilities {
		Facilities(BTreeMap::from([(
			Location::Kitchen,
			FacilityTable {
				card_levels: BTreeMap::from([
					(5, BTreeMap::from([(CardKind::Cut, 4), (CardKind::Ferment, 0)])),
					(6, BTreeMap::from([(CardKind::Cut, 5)])),
				]),
				card_limits: BTreeMap::from([(5, BTreeMap::from([(CardKind::Cut, 3), (CardKind::Ferment, 0)]))]),
				recipe_unlocks: BTreeMap::new(),
			},
		)]))
	}

	#[test]
	fn caps_apply_only_where_known() {
		let facilities = test_facilities();
		let owned = Owned { limit: 4, level: 8 };

		let cut = facilities.cap(Location::Kitchen, 5, CardKind::Cut);
		assert_eq!(cut.apply(owned), Owned { limit: 3, level: 4 });
		assert_eq!(cut.maximum(Owned { limit: 1, level: 1 }), Owned { limit: 3, level: 4 });

		// Level 6 has card levels but no limits
		let cut = facilities.cap(Location::Kitchen, 6, CardKind::Cut);
		assert_eq!(cut.apply(owned), Owned { limit: 4, level: 5 });

		// Cards, levels and locations with no data aren't capped
		assert_eq!(facilities.cap(Location::Kitchen, 5, CardKind::Banquet), Cap::default());
		assert_eq!(facilities.cap(Location::Kitchen, MAX_LEVEL, CardKind::Cut), Cap::default());
		assert_eq!(facilities.cap(Location::Forge, 5, CardKind::Forge), Cap::default());
	}

	#[test]
	fn locked_cards_get_no_copies() {
		let ferment = test_facilities().cap(Location::Kitchen, 5, CardKind::Ferment);
		assert!(ferment.locked());
		assert_eq!(ferment.apply(Owned { limit: 2, level: 2 }).limit, 0);
		assert_eq!(ferment.maximum(Owned { limit: 2, level: 2 }).limit, 0);
	}
}
