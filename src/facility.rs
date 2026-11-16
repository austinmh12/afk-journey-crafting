//! What each facility level allows, loaded from `data/facilities.ron`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::card::CardKind;
use crate::inventory::Owned;
use crate::recipe::Location;

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
mod tests {
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
	fn caps_apply_only_where_known() {
		let facilities = facilities();
		let owned = Owned { limit: 4, level: 8 };

		// Kitchen 10: Cut up to level 6, 3 copies
		let cut = facilities.cap(Location::Kitchen, 10, CardKind::Cut);
		assert_eq!(cut.apply(owned), Owned { limit: 3, level: 6 });
		assert_eq!(cut.maximum(Owned { limit: 1, level: 1 }), Owned { limit: 3, level: 6 });

		// Kitchen 11 has card levels but no limits yet
		let cut = facilities.cap(Location::Kitchen, 11, CardKind::Cut);
		assert_eq!(cut.apply(owned), Owned { limit: 4, level: 6 });

		// Cards and levels with no data aren't capped
		assert_eq!(facilities.cap(Location::Kitchen, 10, CardKind::Banquet), Cap::default());
		assert_eq!(facilities.cap(Location::Kitchen, MAX_LEVEL, CardKind::Cut).apply(owned), owned);
	}

	#[test]
	fn locked_cards_get_no_copies() {
		let facilities = facilities();
		let ferment = facilities.cap(Location::Kitchen, 5, CardKind::Ferment);
		assert!(ferment.locked());
		assert_eq!(ferment.apply(Owned { limit: 2, level: 2 }).limit, 0);
	}
}
