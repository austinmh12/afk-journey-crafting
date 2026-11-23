//! The cards a player owns at each location, and their facility levels.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::card::{Card, CardKind};
use crate::facility::{Cap, Facilities};
use crate::recipe::{Location, Recipe};

/// How many copies of a card may be used, and its current level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Owned {
	pub limit: usize,
	pub level: u8,
}

impl Default for Owned {
	/// A card that isn't owned: level 1, never used.
	fn default() -> Self {
		Self { limit: 0, level: 1 }
	}
}

/// A player's facility at one location, and the cards they own there.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Facility {
	/// The facility's level, which caps card levels and copies (see
	/// [`Facilities`]). `None` leaves the cards uncapped.
	#[serde(default)]
	pub level: Option<u8>,
	#[serde(default)]
	pub cards: BTreeMap<CardKind, Owned>,
}

impl Facility {
	/// What this facility's level allows for a card of `kind`.
	pub fn cap(&self, facilities: &Facilities, location: Location, kind: CardKind) -> Cap {
		self.level.map_or_else(Cap::default, |level| facilities.cap(location, level, kind))
	}

	/// Whether `recipe` is unlocked at this facility's level. Every recipe is
	/// when the level isn't set.
	pub fn recipe_unlocked(&self, facilities: &Facilities, recipe: &Recipe) -> bool {
		self.level.is_none_or(|level| facilities.recipe_unlocked(recipe, level))
	}

	/// The owned card of `kind`, or the default if it isn't listed.
	pub fn owned(&self, kind: CardKind) -> Owned {
		self.cards.get(&kind).copied().unwrap_or_default()
	}

	/// A facility at `level` with every card at `location` at the highest level
	/// and copies that level allows. Cards it hasn't unlocked get no copies.
	pub fn maxed(facilities: &Facilities, location: Location, level: u8) -> Self {
		let mut facility = Self { level: Some(level), cards: BTreeMap::new() };
		for kind in CardKind::ALL.into_iter().filter(|kind| kind.location() == location) {
			let owned = facility.cap(facilities, location, kind).maximum(Owned::default());
			facility.cards.insert(kind, owned);
		}
		facility
	}
}

pub type Inventory = BTreeMap<Location, Facility>;

/// Every location at the facility level `level` gives it, with every card maxed
/// for that level (see [`Facility::maxed`]).
pub fn maxed_inventory(facilities: &Facilities, level: impl Fn(Location) -> u8) -> Inventory {
	Location::ALL
		.into_iter()
		.map(|location| (location, Facility::maxed(facilities, location, level(location))))
		.collect()
}

/// The cards that may be used at `location`, with how many copies of each,
/// capped by the facility's level. Cards with a limit of 0 are left out.
pub fn available_cards(inventory: &Inventory, facilities: &Facilities, location: Location) -> Vec<(Card, usize)> {
	let Some(facility) = inventory.get(&location) else {
		return Vec::new();
	};
	facility
		.cards
		.iter()
		.map(|(&kind, &owned)| (kind, facility.cap(facilities, location, kind).apply(owned)))
		.filter(|(_, owned)| owned.limit > 0)
		.map(|(kind, owned)| (Card { kind, level: owned.level }, owned.limit))
		.collect()
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn available_cards_are_capped_by_facility_level() {
		let facilities = crate::facility::tests::test_facilities();
		let mut facility = Facility {
			level: Some(5),
			cards: BTreeMap::from([
				(CardKind::Cut, Owned { limit: 4, level: 8 }),
				// Not unlocked at level 5
				(CardKind::Ferment, Owned { limit: 2, level: 2 }),
				// No data, so not capped
				(CardKind::Banquet, Owned { limit: 1, level: 3 }),
			]),
		};
		let inventory = |facility: &Facility| Inventory::from([(Location::Kitchen, facility.clone())]);

		let cards = available_cards(&inventory(&facility), &facilities, Location::Kitchen);
		assert_eq!(
			cards,
			vec![
				(Card { kind: CardKind::Cut, level: 4 }, 3),
				(Card { kind: CardKind::Banquet, level: 3 }, 1),
			]
		);

		facility.level = None;
		let cards = available_cards(&inventory(&facility), &facilities, Location::Kitchen);
		assert_eq!(cards.len(), 3);
	}

	#[test]
	fn maxed_inventory_uses_each_facility_level() {
		let facilities: Facilities = ron::from_str(include_str!("../data/facilities.ron")).unwrap();
		let inventory = maxed_inventory(&facilities, |location| if location == Location::Kitchen { 2 } else { 1 });
		assert_eq!(inventory.len(), Location::ALL.len());

		// Kitchen level 2: Heat Control unlocks, Cut is level 2 with 3 copies
		let kitchen = available_cards(&inventory, &facilities, Location::Kitchen);
		assert_eq!(
			kitchen,
			vec![
				(Card { kind: CardKind::HeatControl, level: 1 }, 1),
				(Card { kind: CardKind::Cut, level: 2 }, 3),
				(Card { kind: CardKind::Season, level: 1 }, 1),
			]
		);
		// Forge level 1: only Forge and Ignite
		let forge = available_cards(&inventory, &facilities, Location::Forge);
		assert_eq!(
			forge,
			vec![(Card { kind: CardKind::Forge, level: 1 }, 3), (Card { kind: CardKind::Ignite, level: 1 }, 1)]
		);
	}
}
