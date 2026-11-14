use std::fmt;

use rand::{Rng, RngExt};
use serde::Deserialize;

use crate::score::Score;
use crate::tables::{AddSub, CardTables};

/// Every card type. Variant order sets the order cards are listed and combined in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
pub enum CardKind {
	// Kitchen
	HeatControl,
	Cut,
	Season,
	SlowCook,
	Ferment,
	Bake,
	ExactCook,
	Banquet,
	// Forge
	Forge,
	Charge,
	Sharpening,
	MultiForge,
	HeatUp,
	Reforge,
	Soulbind,
	ForgeExpert,
	Pyroburst,
	Ignite,
	// Alchemy
	Ingredients,
	Grind,
	Distill,
	Crystallize,
	Overload,
	Enchant,
	Fuse,
	Reversal,
	Conflux,
	Harmonia,
	Overcharge,
}

impl CardKind {
	pub fn name(self) -> &'static str {
		match self {
			Self::HeatControl => "Heat Control",
			Self::Cut => "Cut",
			Self::Season => "Season",
			Self::SlowCook => "Slow Cook",
			Self::Ferment => "Ferment",
			Self::Bake => "Bake",
			Self::ExactCook => "Exact Cook",
			Self::Banquet => "Banquet",
			Self::Forge => "Forge",
			Self::Charge => "Charge",
			Self::Sharpening => "Sharpening",
			Self::MultiForge => "Multi-Forge",
			Self::HeatUp => "Heat Up",
			Self::Reforge => "Reforge",
			Self::Soulbind => "Soulbind",
			Self::ForgeExpert => "Forge Expert",
			Self::Pyroburst => "Pyroburst",
			Self::Ignite => "Ignite",
			Self::Ingredients => "Ingredients",
			Self::Grind => "Grind",
			Self::Distill => "Distill",
			Self::Crystallize => "Crystallize",
			Self::Overload => "Overload",
			Self::Enchant => "Enchant",
			Self::Fuse => "Fuse",
			Self::Reversal => "Reversal",
			Self::Conflux => "Conflux",
			Self::Harmonia => "Harmonia",
			Self::Overcharge => "Overcharge",
		}
	}
}

/// A card at a specific level. A deck only ever holds one level of each kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Card {
	pub kind: CardKind,
	pub level: u8,
}

impl fmt::Display for Card {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{} (lvl {})", self.kind.name(), self.level)
	}
}

/// Heat Control can retrigger at most this many times from its 50% chance.
const HEAT_CONTROL_RETRIGGER_LIMIT: u32 = 7;

impl Card {
	/// Adds this card's effect to `score`.
	///
	/// `context` is normally the cards played before this one. Exact Cook receives
	/// the whole play order and Sharpening receives the cards still to be played.
	pub fn score<R: Rng + ?Sized>(
		&self,
		score: &mut Score,
		recipe: &str,
		context: &[Card],
		tables: &CardTables,
		rng: &mut R,
	) {
		let level = self.level;
		match self.kind {
			// ---------- Kitchen ----------
			CardKind::Cut => {
				let range = tables.cut.get(level);
				let hit = |score: &mut Score, rng: &mut R| {
					// Salted Raisin power: cut always rolls its max value
					let value = if recipe == "Salted Raisin" {
						range.max
					} else {
						rng.random_range(range.min..=range.max)
					};
					// Roasted Corn power: cut always hits the lowest colour
					if recipe == "Roasted Corn" {
						score.add_lowest(value);
					} else {
						score.add_random(value, rng);
					}
				};
				hit(score, rng);
				// Retrigger once per Cut directly before this one, twice at lvl 9+
				for prev in context.iter().rev() {
					if prev.kind != CardKind::Cut {
						break;
					}
					hit(score, rng);
					if level >= 9 {
						hit(score, rng);
					}
				}
			}
			CardKind::HeatControl => {
				let value = tables.heat_control.get(level);
				score.add_random(value, rng);

				// Slow Cook triggers when Heat Control flips, and at lvl 4 and below
				// also on every Heat Control retrigger
				let slow_cooks: Vec<&Card> = context.iter().filter(|c| c.kind == CardKind::SlowCook).collect();
				let slow_cook_level = slow_cooks.first().map_or(0, |c| c.level);
				let mut slow_cook_triggers = u32::from(!slow_cooks.is_empty());

				// Each Ferment makes Heat Control trigger extra times based on its level
				let ferments: Vec<&Card> = context.iter().filter(|c| c.kind == CardKind::Ferment).collect();
				let ferment_level = ferments.first().map_or(0, |c| c.level);
				for _ in 0..ferments.len() * usize::from(ferment_level) {
					score.add_random(value, rng);
					if slow_cook_level < 5 {
						slow_cook_triggers += 1;
					}
				}

				// 50% chance to keep retriggering
				let mut limit = HEAT_CONTROL_RETRIGGER_LIMIT;
				while rng.random::<f64>() >= 0.5 && limit > 0 {
					score.add_random(value, rng);
					if slow_cook_level < 5 {
						slow_cook_triggers += 1;
					}
					limit -= 1;
				}

				for slow_cook in slow_cooks {
					for _ in 0..slow_cook_triggers {
						// Recipe and context don't affect Slow Cook
						slow_cook.score(score, "", &[], tables, rng);
					}
				}
			}
			CardKind::Season => score.double_random(rng),
			CardKind::SlowCook => {
				let value = tables.slow_cook.get(level);
				// At lvl 2+ it hits every colour
				if level >= 2 {
					score.add_all(value);
				} else {
					score.add_random(value, rng);
				}
			}
			CardKind::Ferment => {} // Only boosts Heat Control
			// Replaced by the card it flips before scoring (see `deck::score_order`).
			// Only reached when Banquet is played first, with nothing to flip.
			CardKind::Banquet => {}
			CardKind::Bake => {
				score.add_random(tables.bake.get(level), rng);
				score.redistribute = true;
			}
			CardKind::ExactCook => {
				// Double at the end if enough identical cards are played in a row
				let needed = tables.exact_cook.get(level);
				let mut prev: Option<CardKind> = None;
				let mut in_a_row = 0;
				for card in context {
					if prev != Some(card.kind) {
						in_a_row = 0;
					} else {
						in_a_row += 1;
						if in_a_row >= needed {
							score.post_double = true;
							break;
						}
					}
					prev = Some(card.kind);
				}
			}

			// ---------- Forge ----------
			CardKind::Forge => {
				let value = tables.forge.get(level) + score.stack.heat_up_bonus;
				score.forge_hit(value, rng);
				while score.stack.multi_forge_amt > 0 {
					score.forge_hit(value, rng);
					score.stack.multi_forge_amt -= 1;
				}
			}
			CardKind::Charge => {
				let charge = tables.charge.get(level);
				score.add_random(charge.points, rng);
				score.stack.charge_charges = charge.charges;
			}
			CardKind::Sharpening => {
				// One hit per card still to be played
				let value = tables.sharpening.get(level);
				for _ in context {
					score.add_random(value, rng);
				}
			}
			CardKind::MultiForge => {
				// Stacks: 2 Multi-Forge + Forge on Copper Carver gave 5 triggers
				score.stack.multi_forge_amt += tables.multi_forge.get(level);
			}
			CardKind::HeatUp => {
				// Permanent, and stacks with other Heat Ups
				score.stack.heat_up_bonus += tables.heat_up.get(level);
			}
			CardKind::Reforge => score.add_all(tables.reforge.get(level)),
			CardKind::Soulbind => {
				// Gets every enhancement from earlier cards: the Heat Up bonus and the
				// current Forge Expert bonus (without raising either), and it refreshes
				// Charge and Multi-Forge before using them up like a forge hit
				let multi_forges: Vec<&Card> = context.iter().filter(|c| c.kind == CardKind::MultiForge).collect();
				if !multi_forges.is_empty() {
					score.stack.multi_forge_amt = multi_forges.iter().map(|c| tables.multi_forge.get(c.level)).sum();
				}
				if let Some(charge) = context.iter().rev().find(|c| c.kind == CardKind::Charge) {
					score.stack.charge_charges = tables.charge.get(charge.level).charges;
				}
				let forge_expert_bonus = context
					.iter()
					.find(|c| c.kind == CardKind::ForgeExpert)
					.map_or(0, |c| tables.forge_expert.get(c.level).bonus * score.stack.forge_expert_mult);
				let value = tables.soulbind.get(level) + score.stack.heat_up_bonus + forge_expert_bonus;
				score.forge_hit(value, rng);
				while score.stack.multi_forge_amt > 0 {
					score.forge_hit(value, rng);
					score.stack.multi_forge_amt -= 1;
				}
			}
			CardKind::ForgeExpert => {
				let expert = tables.forge_expert.get(level);
				let value = expert.value + score.stack.heat_up_bonus;
				// Each hit, including Multi-Forge hits, grows the bonus multiplier
				score.forge_hit(value + expert.bonus * score.stack.forge_expert_mult, rng);
				score.stack.forge_expert_mult += 1;
				while score.stack.multi_forge_amt > 0 {
					score.forge_hit(value + expert.bonus * score.stack.forge_expert_mult, rng);
					score.stack.multi_forge_amt -= 1;
					score.stack.forge_expert_mult += 1;
				}
			}
			CardKind::Pyroburst => {
				let value = tables.pyroburst.get(level) + score.stack.heat_up_bonus;
				// Two or more active boosts (Heat Up, Charge, Multi-Forge) give 2 extra hits
				let mut boosts = u32::from(score.stack.heat_up_bonus != 0);
				if score.stack.charge_charges != 0 {
					boosts += 1;
				}
				score.forge_hit(value, rng);
				if score.stack.multi_forge_amt != 0 {
					boosts += 1;
				}
				while score.stack.multi_forge_amt > 0 {
					score.forge_hit(value, rng);
					score.stack.multi_forge_amt -= 1;
				}
				if boosts >= 2 {
					score.forge_hit(value, rng);
					score.forge_hit(value, rng);
				}
			}
			CardKind::Ignite => score.double_random(rng),

			// ---------- Alchemy ----------
			CardKind::Overcharge => {
				let overcharge = tables.overcharge.get(level);
				score.stack.overcharge_amt = overcharge.amount;
				score.stack.overcharge_mult = overcharge.mult;
			}
			CardKind::Fuse => {
				let fuse = tables.fuse.get(level);
				score.stack.fuse_bonus = fuse.bonus;
				score.stack.fuse_gap = fuse.gap;
			}
			CardKind::Ingredients => {
				let AddSub { add, sub } = tables.ingredients.get(level);
				score.add_highest(add);
				if score.stack.reversal {
					// Reversal turns the decrease into a multiplied increase on the highest
					score.add_highest(-(score.stack.reversal_mult * sub));
				} else if !score.stack.harmonia {
					// Harmonia blocks the decrease to the lowest colour
					score.add_lowest(sub);
				}
				score.apply_debuffs();
			}
			CardKind::Reversal => {
				score.stack.reversal = true;
				score.stack.reversal_mult = tables.reversal.get(level);
			}
			CardKind::Enchant => {
				let enchant = tables.enchant.get(level);
				score.add_lowest(enchant.value);
				score.apply_debuffs();
				// The debuff only applies to later cards
				score.stack.enchant_debuff += enchant.debuff;
			}
			CardKind::Overload => {
				let overload = tables.overload.get(level);
				score.add_highest(overload.value);
				score.apply_debuffs();
				// The debuff only applies to later cards
				score.stack.overload_debuff += overload.debuff;
			}
			CardKind::Harmonia => score.stack.harmonia = true,
			CardKind::Conflux => score.add_highest(score.lowest().div_euclid(2)),
			CardKind::Crystallize => score.double_lowest(),
			CardKind::Distill => score.double_highest(),
			CardKind::Grind => {
				let AddSub { add, sub } = tables.grind.get(level);
				score.add_highest(sub);
				if score.stack.overcharge_amt != 0 {
					score.stack.overcharge_amt -= 1;
					score.add_highest(add * score.stack.overcharge_mult);
				} else {
					score.add_lowest(add);
				}
				score.apply_debuffs();
			}
		}
	}
}
