//! What each card does, when it unlocks and upgrades, and its values at each
//! level, for the web page's card guide; and what unlocks at each facility
//! level, for its facility guide.

use crate::card::CardKind;
use crate::facility::{Facilities, MAX_LEVEL};
use crate::recipe::Location;
use crate::tables::{AddSub, CardTables, Table};

/// How the card scores, or what it does to other cards.
pub fn description(kind: CardKind) -> &'static str {
	match kind {
		// Kitchen
		CardKind::HeatControl => {
			"Adds its points to a random colour, then has a 50% chance to trigger again, over and over with no limit. Ferment gives it extra triggers, and Slow Cook played before it triggers along with it."
		}
		CardKind::Cut => {
			"Adds a random number of points from its range to a random colour. From level 7 it triggers again once for each Cut played in a row directly before it, or twice for each from level 9."
		}
		CardKind::Season => "Doubles a random colour.",
		CardKind::SlowCook => {
			"Adds its points when flipped: to a random colour at level 1, and to both colours from level 2. Each Heat Control played after it also triggers it: up to level 4 on every Heat Control trigger, and from level 5 only once per Heat Control flip."
		}
		CardKind::Ferment => {
			"Scores nothing itself. Each Heat Control played after it triggers extra times: the first Ferment's level, for each Ferment played before it."
		}
		CardKind::Bake => {
			"Adds its points to a random colour. At the end of the craft, both colours are set to their average."
		}
		CardKind::ExactCook => {
			"At the end of the craft, doubles a random colour if the play order has enough identical cards in a row anywhere in it."
		}
		CardKind::Banquet => {
			"Flips the card played before it again, and later cards treat it as that card (so it can carry on a Cut streak). Played first, it does nothing."
		}
		// Forge
		CardKind::Forge => {
			"A forge hit: adds its points, plus any Heat Up bonus, to a random colour, or to both colours while Charge has charges. Multi-Forge gives it extra hits."
		}
		CardKind::Charge => {
			"Adds its points to a random colour. Each later forge hit (Forge, Forge Expert, Pyroburst, Soulbind) hits both colours instead of one, using up a charge."
		}
		CardKind::Sharpening => "Adds its points to a random colour once for every card played after it.",
		CardKind::MultiForge => {
			"The next forge card hits extra times. Multi-Forges stack, and each extra hit gets Heat Up and Charge like a normal hit."
		}
		CardKind::HeatUp => "Every later forge hit gains its bonus. Stacks with other Heat Ups.",
		CardKind::Reforge => {
			"Scores nothing itself. After every later Forge, Forge Expert and Pyroburst hit, extra hits included, it adds its points to both colours. Stacks with other Reforges, and counts as an enhancement for those cards."
		}
		CardKind::Soulbind => {
			"A forge hit that gets every enhancement from earlier cards: the Heat Up bonus, the current Forge Expert bonus, and Charge and Multi-Forge, refreshed from the cards played before it."
		}
		CardKind::ForgeExpert => {
			"A forge hit worth its points plus its bonus for every earlier Forge Expert hit in the craft, so it grows with each hit, Multi-Forge hits included."
		}
		CardKind::Pyroburst => {
			"A forge hit. Enhanced by 2 or more of Heat Up, Charge and Multi-Forge, it hits twice more."
		}
		CardKind::Ignite => "Doubles a random colour.",
		// Alchemy
		CardKind::Ingredients => {
			"Adds to the highest colour and takes from the lowest. Reversal turns the decrease into an increase on the highest colour, times its multiplier; Harmonia blocks it. Then any debuffs apply."
		}
		CardKind::Grind => {
			"Takes from the highest colour and adds to the lowest. While Overcharge has uses left, the addition goes to the highest colour instead, multiplied. Then any debuffs apply."
		}
		CardKind::Distill => "Doubles the highest colour.",
		CardKind::Crystallize => "Doubles the lowest colour.",
		CardKind::Overload => {
			"Adds to the highest colour, then any debuffs apply. Its own debuff then lowers a random colour at level 1, or both colours from level 2, after every later Ingredients, Grind, Enchant and Overload."
		}
		CardKind::Enchant => {
			"Adds to the lowest colour, then any debuffs apply. Its own debuff then lowers a random colour at level 1, or both colours from level 2, after every later Ingredients, Grind, Enchant and Overload."
		}
		CardKind::Fuse => {
			"At the end of the craft, both colours gain its bonus if they're less than its gap apart."
		}
		CardKind::Reversal => {
			"Turns later decreases into increases on the highest colour: Ingredients' decrease (times its multiplier) and the Enchant and Overload debuffs."
		}
		CardKind::Conflux => "Adds half the lowest colour to the highest.",
		CardKind::Harmonia => {
			"From then on only the highest colour can decrease: Ingredients no longer takes from the lowest, and debuffs only lower the highest."
		}
		CardKind::Overcharge => {
			"The next few Grinds add to the highest colour, multiplied, instead of to the lowest."
		}
	}
}

/// Whether the card changes a colour itself, rather than only changing how
/// other cards score.
pub fn scores(kind: CardKind) -> bool {
	!matches!(
		kind,
		CardKind::Ferment
			| CardKind::Banquet
			| CardKind::Charge
			| CardKind::MultiForge
			| CardKind::HeatUp
			| CardKind::Reversal
			| CardKind::Harmonia
			| CardKind::Overcharge
	)
}

/// The cards at `location` in the order the card guide lists them: by the
/// facility level they unlock at, with cards that score before cards that
/// don't, then in card order. Cards with no known unlock level go last.
pub fn guide_order(facilities: &Facilities, location: Location) -> Vec<CardKind> {
	let mut kinds: Vec<CardKind> = CardKind::ALL.into_iter().filter(|kind| kind.location() == location).collect();
	// Stable, so ties keep card order
	kinds.sort_by_cached_key(|&kind| {
		let unlock = unlock_level(&progression(facilities, kind)).unwrap_or(u8::MAX);
		(unlock, !scores(kind))
	});
	kinds
}

/// What changed for one card at a facility level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CardChange {
	pub kind: CardKind,
	/// Whether the card unlocked at this level.
	pub new: bool,
	/// The card's new highest level, if it changed.
	pub card_level: Option<u8>,
	/// The card's new copies per deck, if they changed.
	pub limit: Option<usize>,
}

/// What unlocks at one facility level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LevelUnlocks {
	pub level: u8,
	/// Recipe names, as listed in the facilities data.
	pub recipes: Vec<String>,
	/// New cards first, then upgrades, each in card guide order.
	pub cards: Vec<CardChange>,
}

/// What unlocks at each facility level at `location`, from level 1 to the highest.
pub fn facility_levels(facilities: &Facilities, location: Location) -> Vec<LevelUnlocks> {
	let mut levels: Vec<LevelUnlocks> = (1..=MAX_LEVEL)
		.map(|level| LevelUnlocks {
			level,
			recipes: facilities
				.0
				.get(&location)
				.and_then(|table| table.recipe_unlocks.get(&level))
				.cloned()
				.unwrap_or_default(),
			cards: Vec::new(),
		})
		.collect();
	for kind in guide_order(facilities, location) {
		// The values the card last had while unlocked
		let mut unlocked: Option<Step> = None;
		for step in progression(facilities, kind) {
			if step.card_level == 0 || step.limit == 0 {
				continue;
			}
			let change = CardChange {
				kind,
				new: unlocked.is_none(),
				card_level: Some(step.card_level).filter(|&level| unlocked.is_none_or(|prev| prev.card_level != level)),
				limit: Some(step.limit).filter(|&limit| unlocked.is_none_or(|prev| prev.limit != limit)),
			};
			unlocked = Some(step);
			if change.card_level.is_some() || change.limit.is_some() {
				levels[usize::from(step.facility_level) - 1].cards.push(change);
			}
		}
	}
	for level in &mut levels {
		// Stable, so each group keeps card guide order
		level.cards.sort_by_key(|change| !change.new);
	}
	levels
}

/// A facility level where a card's level cap or copies per deck changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Step {
	pub facility_level: u8,
	/// The highest card level. 0 while the card is locked.
	pub card_level: u8,
	/// The most copies in one deck. 0 while the card is locked.
	pub limit: usize,
}

/// Each facility level where `kind`'s level cap or copies change, lowest first,
/// from the facilities data. Levels with no data keep the previous values.
pub fn progression(facilities: &Facilities, kind: CardKind) -> Vec<Step> {
	let Some(table) = facilities.0.get(&kind.location()) else {
		return Vec::new();
	};
	let mut steps: Vec<Step> = Vec::new();
	let (mut card_level, mut limit) = (0, 0);
	for facility_level in 1..=MAX_LEVEL {
		let level_cap = table.card_levels.get(&facility_level).and_then(|cards| cards.get(&kind));
		let limit_cap = table.card_limits.get(&facility_level).and_then(|cards| cards.get(&kind));
		card_level = level_cap.copied().unwrap_or(card_level);
		limit = limit_cap.copied().unwrap_or(limit);
		let changed = steps.last().map_or(card_level > 0 || limit > 0, |last| {
			(last.card_level, last.limit) != (card_level, limit)
		});
		if changed {
			steps.push(Step { facility_level, card_level, limit });
		}
	}
	steps
}

/// The facility level `kind` unlocks at: the first with a card level and copies.
pub fn unlock_level(steps: &[Step]) -> Option<u8> {
	steps.iter().find(|step| step.card_level > 0 && step.limit > 0).map(|step| step.facility_level)
}

/// A card's values at each card level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Values {
	/// One heading per value column.
	pub columns: Vec<&'static str>,
	/// One row per card level, lowest first.
	pub rows: Vec<Row>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
	pub level: u8,
	/// One cell per column.
	pub cells: Vec<String>,
}

/// `kind`'s values for card levels 1 to `max_level`. A card with no values
/// (Season, Ignite and so on) gets no columns.
pub fn values(tables: &CardTables, kind: CardKind, max_level: u8) -> Values {
	let (columns, cell):(Vec<&'static str>, Cell<'_>) = match kind {
		CardKind::HeatControl => (vec!["Points"], number(&tables.heat_control)),
		CardKind::Cut => (
			vec!["Points", "Extra triggers per Cut before it"],
			Box::new(|level| {
				let range = tables.cut.get(level);
				let points = if range.min == range.max {
					range.min.to_string()
				} else {
					format!("{}–{}", range.min, range.max)
				};
				vec![points, crate::card::cut_retriggers(level).to_string()]
			}),
		),
		CardKind::SlowCook => (
			vec!["Points", "Adds to", "Triggered by Heat Control"],
			Box::new(|level| {
				let points = tables.slow_cook.get(level);
				let adds_to = if level >= 2 { "Both colours" } else { "Random colour" };
				let triggers = if level >= 5 { "Once per flip" } else { "Every trigger" };
				vec![points.to_string(), adds_to.into(), triggers.into()]
			}),
		),
		CardKind::Ferment => (
			vec!["Extra Heat Control triggers"],
			Box::new(|level| vec![level.to_string()]),
		),
		CardKind::Bake => (vec!["Points"], number(&tables.bake)),
		CardKind::ExactCook => (
			vec!["Identical cards in a row"],
			Box::new(|level| {
				// Counts repeats after the first card of the run
				let repeats = tables.exact_cook.get(level);
				vec![(repeats + 1).to_string()]
			}),
		),
		CardKind::Banquet => (vec!["Flips"], number(&tables.banquet)),
		CardKind::Forge => (vec!["Points per hit"], number(&tables.forge)),
		CardKind::Charge => (
			vec!["Points", "Charges"],
			Box::new(|level| {
				let charge = tables.charge.get(level);
				// 99 stands for unlimited (see data/cards.ron)
				let charges = if charge.charges >= 99 { "Unlimited".into() } else { charge.charges.to_string() };
				vec![charge.points.to_string(), charges]
			}),
		),
		CardKind::Sharpening => (vec!["Points per later card"], number(&tables.sharpening)),
		CardKind::MultiForge => (vec!["Extra hits"], number(&tables.multi_forge)),
		CardKind::HeatUp => (vec!["Bonus per hit"], signed(&tables.heat_up)),
		CardKind::Reforge => (vec!["Points to both colours per hit"], number(&tables.reforge)),
		CardKind::Soulbind => (vec!["Points"], number(&tables.soulbind)),
		CardKind::ForgeExpert => (
			vec!["Points", "Bonus per earlier hit"],
			Box::new(|level| {
				let expert = tables.forge_expert.get(level);
				vec![expert.value.to_string(), format!("+{}", expert.bonus)]
			}),
		),
		CardKind::Pyroburst => (vec!["Points per hit"], number(&tables.pyroburst)),
		CardKind::Ingredients => (vec!["Highest colour", "Lowest colour"], add_sub(&tables.ingredients)),
		CardKind::Grind => (vec!["Lowest colour", "Highest colour"], add_sub(&tables.grind)),
		CardKind::Overload => (
			vec!["Highest colour", "Debuff"],
			Box::new(|level| {
				let overload = tables.overload.get(level);
				vec![format!("{:+}", overload.value), format!("−{}", overload.debuff)]
			}),
		),
		CardKind::Enchant => (
			vec!["Lowest colour", "Debuff"],
			Box::new(|level| {
				let enchant = tables.enchant.get(level);
				vec![format!("{:+}", enchant.value), format!("−{}", enchant.debuff)]
			}),
		),
		CardKind::Fuse => (
			vec!["Bonus", "Gap under"],
			Box::new(|level| {
				let fuse = tables.fuse.get(level);
				vec![format!("+{}", fuse.bonus), fuse.gap.to_string()]
			}),
		),
		CardKind::Reversal => (
			vec!["Multiplier"],
			Box::new(|level| {
				let mult = tables.reversal.get(level);
				vec![format!("×{mult}")]
			}),
		),
		CardKind::Overcharge => (
			vec!["Grinds boosted", "Multiplier"],
			Box::new(|level| {
				let overcharge = tables.overcharge.get(level);
				vec![overcharge.amount.to_string(), format!("×{}", overcharge.mult)]
			}),
		),
		CardKind::Season
		| CardKind::Ignite
		| CardKind::Distill
		| CardKind::Crystallize
		| CardKind::Conflux
		| CardKind::Harmonia => (Vec::new(), Box::new(|_| Vec::new())),
	};
	let rows = (1..=max_level)
		.map(|level| Row { level, cells: cell(level) })
		.collect();
	Values { columns, rows }
}

type Cell<'a> = Box<dyn Fn(u8) -> Vec<String> + 'a>;

fn number(table: &Table<impl Copy + ToString>) -> Cell<'_> {
	Box::new(|level| {
		let value = table.get(level);
		vec![value.to_string()]
	})
}

fn signed(table: &Table<i64>) -> Cell<'_> {
	Box::new(|level| {
		let value = table.get(level);
		vec![format!("{value:+}")]
	})
}

/// `add` then `sub`, as signed changes. Ingredients adds to the highest colour
/// and subtracts from the lowest; Grind the other way round.
fn add_sub(table: &Table<AddSub>) -> Cell<'_> {
	Box::new(|level| {
		let AddSub { add, sub } = table.get(level);
		vec![format!("{add:+}"), format!("{sub:+}")]
	})
}

/// The highest level `kind` can reach, from the facilities data, or else the
/// highest level `cards.ron` lists for it. At least 1.
pub fn max_level(facilities: &Facilities, tables: &CardTables, kind: CardKind) -> u8 {
	let from_facilities = progression(facilities, kind).iter().map(|step| step.card_level).max();
	from_facilities.filter(|&level| level > 0).unwrap_or_else(|| listed_max(tables, kind)).max(1)
}

fn listed_max(tables: &CardTables, kind: CardKind) -> u8 {
	fn last<T>(table: &Table<T>) -> u8 {
		table.levels.keys().next_back().copied().unwrap_or(1)
	}
	match kind {
		CardKind::HeatControl => last(&tables.heat_control),
		CardKind::Cut => last(&tables.cut),
		CardKind::SlowCook => last(&tables.slow_cook),
		CardKind::Bake => last(&tables.bake),
		CardKind::ExactCook => last(&tables.exact_cook),
		CardKind::Banquet => last(&tables.banquet),
		CardKind::Forge => last(&tables.forge),
		CardKind::Charge => last(&tables.charge),
		CardKind::Sharpening => last(&tables.sharpening),
		CardKind::MultiForge => last(&tables.multi_forge),
		CardKind::HeatUp => last(&tables.heat_up),
		CardKind::Reforge => last(&tables.reforge),
		CardKind::Soulbind => last(&tables.soulbind),
		CardKind::ForgeExpert => last(&tables.forge_expert),
		CardKind::Pyroburst => last(&tables.pyroburst),
		CardKind::Overcharge => last(&tables.overcharge),
		CardKind::Fuse => last(&tables.fuse),
		CardKind::Ingredients => last(&tables.ingredients),
		CardKind::Reversal => last(&tables.reversal),
		CardKind::Enchant => last(&tables.enchant),
		CardKind::Overload => last(&tables.overload),
		CardKind::Grind => last(&tables.grind),
		_ => 1,
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn facilities() -> Facilities {
		ron::from_str(include_str!("../data/facilities.ron")).unwrap()
	}

	fn tables() -> CardTables {
		ron::from_str(include_str!("../data/cards.ron")).unwrap()
	}

	#[test]
	fn heat_control_progression() {
		let steps = progression(&facilities(), CardKind::HeatControl);
		assert_eq!(steps[0], Step { facility_level: 2, card_level: 1, limit: 1 });
		assert_eq!(steps[1], Step { facility_level: 3, card_level: 1, limit: 2 });
		assert_eq!(unlock_level(&steps), Some(2));
		let last = steps.last().unwrap();
		assert_eq!((last.facility_level, last.card_level, last.limit), (17, 8, 4));
	}

	#[test]
	fn every_card_has_rows_up_to_its_max_level() {
		let (facilities, tables) = (facilities(), tables());
		for kind in CardKind::ALL {
			let max = max_level(&facilities, &tables, kind);
			let values = values(&tables, kind, max);
			assert_eq!(values.rows.len(), usize::from(max), "{kind:?}");
			for row in &values.rows {
				assert_eq!(row.cells.len(), values.columns.len(), "{kind:?} level {}", row.level);
			}
			assert!(unlock_level(&progression(&facilities, kind)).is_some(), "{kind:?} never unlocks");
		}
	}

	#[test]
	fn guide_order_by_unlock_level_then_scoring() {
		use CardKind::*;
		let facilities = facilities();
		// Forge and Ignite unlock at facility level 1, Heat Up at 3
		assert_eq!(&guide_order(&facilities, Location::Forge)[..3], [Forge, Ignite, HeatUp]);
		// Both score and unlock at 1, so they keep card order
		assert_eq!(&guide_order(&facilities, Location::Kitchen)[..2], [Cut, Season]);
		// Every card is listed once
		for location in Location::ALL {
			let order = guide_order(&facilities, location);
			assert_eq!(order.len(), CardKind::ALL.iter().filter(|kind| kind.location() == location).count());
		}
	}

	#[test]
	fn ties_put_scoring_cards_first() {
		use std::collections::BTreeMap;
		use crate::facility::FacilityTable;
		use CardKind::*;
		// Heat Up (doesn't score) and Reforge (scores) both unlock at level 1
		let caps = BTreeMap::from([(HeatUp, 1), (Reforge, 1), (Forge, 0)]);
		let facilities = Facilities(BTreeMap::from([(
			Location::Forge,
			FacilityTable {
				card_levels: BTreeMap::from([(1, caps.clone())]),
				card_limits: BTreeMap::from([(1, caps.into_iter().map(|(kind, level)| (kind, usize::from(level))).collect())]),
				recipe_unlocks: BTreeMap::new(),
			},
		)]));
		let order = guide_order(&facilities, Location::Forge);
		assert_eq!(&order[..2], [Reforge, HeatUp]);
	}

	#[test]
	fn facility_level_unlocks() {
		use CardKind::*;
		let levels = facility_levels(&facilities(), Location::Kitchen);
		assert_eq!(levels.len(), usize::from(MAX_LEVEL));
		let first = &levels[0];
		assert_eq!(first.recipes, ["Round Bread"]);
		assert_eq!(
			first.cards,
			[
				CardChange { kind: Cut, new: true, card_level: Some(1), limit: Some(2) },
				CardChange { kind: Season, new: true, card_level: Some(1), limit: Some(1) },
			]
		);
		// Heat Control unlocks at 2, while Cut levels up and gains a copy
		assert_eq!(
			levels[1].cards,
			[
				CardChange { kind: HeatControl, new: true, card_level: Some(1), limit: Some(1) },
				CardChange { kind: Cut, new: false, card_level: Some(2), limit: Some(3) },
			]
		);
		// Heat Control only gains a copy at 3
		assert!(levels[2].cards.contains(&CardChange { kind: HeatControl, new: false, card_level: None, limit: Some(2) }));
	}

	#[test]
	fn value_cells() {
		let tables = tables();
		let cut = values(&tables, CardKind::Cut, 9);
		assert_eq!(cut.rows[0].cells, ["1", "0"]);
		assert_eq!(cut.rows[5].cells[1], "0");
		assert_eq!(cut.rows[6].cells[1], "1");
		assert_eq!(cut.rows[8].cells, ["13–18", "2"]);
		let charge = values(&tables, CardKind::Charge, 4);
		assert_eq!(charge.rows[3].cells, ["5", "Unlimited"]);
		let exact = values(&tables, CardKind::ExactCook, 2);
		assert_eq!(exact.rows[1].cells, ["3"]);
	}
}
