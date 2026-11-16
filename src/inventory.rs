//! The cards a player owns, as stored in `data/inventory.ron`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::card::{Card, CardKind};
use crate::recipe::Location;

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

pub type Inventory = BTreeMap<Location, BTreeMap<CardKind, Owned>>;

/// The cards that may be used at `location`, with how many copies of each.
/// Cards with a limit of 0 are left out.
pub fn available_cards(inventory: &Inventory, location: Location) -> Vec<(Card, usize)> {
	inventory
		.get(&location)
		.into_iter()
		.flatten()
		.filter(|(_, owned)| owned.limit > 0)
		.map(|(&kind, owned)| (Card { kind, level: owned.level }, owned.limit))
		.collect()
}
