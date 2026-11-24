use std::fmt;

use rand::{Rng, RngExt};

/// Persistent modifiers that cards leave behind for later cards in the same craft.
#[derive(Debug, Clone, Copy, Default)]
pub struct Stack {
	// Kitchen has no modifiers
	// Forge modifiers
	pub charge_charges: i64,
	pub multi_forge_amt: i64,
	pub heat_up_bonus: i64,
	pub forge_expert_mult: i64,
	// Alchemy modifiers
	pub overcharge_amt: i64,
	pub overcharge_mult: i64,
	pub reversal: bool,
	pub reversal_mult: i64,
	pub enchant_debuff: Debuff,
	pub overload_debuff: Debuff,
	pub harmonia: bool,
	pub fuse_bonus: i64,
	pub fuse_gap: i64,
}

/// The debuff left by every Enchant, or every Overload, played so far. Every copy
/// of a card in a deck is the same level, so they all debuff the same way.
#[derive(Debug, Clone, Copy, Default)]
pub struct Debuff {
	/// How much each card lowers a colour by.
	pub value: i64,
	/// How many of the card have been played.
	pub count: i64,
	/// Each card's debuff hits a random colour (level 1) instead of both.
	pub random: bool,
}

impl Debuff {
	/// Adds a card's debuff, for the cards played after it.
	pub fn add(&mut self, value: i64, random: bool) {
		self.value = value;
		self.count += 1;
		self.random = random;
	}
}

/// The two colour values of a craft. The final score is `left * right`.
#[derive(Debug, Clone, Copy)]
pub struct Score {
	pub left: i64,
	pub right: i64,
	pub stack: Stack,
	/// Set by Bake: average both colours once all cards are played.
	pub redistribute: bool,
	/// Set by Exact Cook: double a random colour once all cards are played.
	pub post_double: bool,
	/// Neither colour may drop below this value. 1 for Alchemy, unlimited elsewhere.
	pub floor: i64,
	/// Times Heat Control has added points, including Ferment and chance retriggers.
	pub heat_control_triggers: u32,
	/// Hits from artisan cards (Forge, Forge Expert, Pyroburst), including extra hits.
	pub artisan_triggers: u32,
	/// Times every effect on the highest colour is applied. 2 for Earth Antidote.
	pub highest_repeats: u32,
}

impl Default for Score {
	fn default() -> Self {
		Self {
			left: 1,
			right: 1,
			stack: Stack::default(),
			redistribute: false,
			post_double: false,
			floor: i64::MIN,
			heat_control_triggers: 0,
			artisan_triggers: 0,
			highest_repeats: 1,
		}
	}
}

impl Score {
	pub fn lowest(&self) -> i64 {
		self.left.min(self.right)
	}

	pub fn total(&self) -> i64 {
		self.left * self.right
	}

	/// Applies the cards' end-of-craft effects. Call once after the last card is scored,
	/// before the recipe's power (see `Recipe::apply_end_power`).
	pub fn finalize<R: Rng + ?Sized>(&mut self, rng: &mut R) {
		if self.post_double {
			self.double_random(rng);
		}
		if self.redistribute {
			let avg = (self.left + self.right).div_euclid(2);
			self.left = avg;
			self.right = avg;
		}
		// Fuse: both colours gain the bonus if they end close enough together
		if self.stack.fuse_bonus != 0 && (self.left - self.right).abs() < self.stack.fuse_gap {
			self.add_all(self.stack.fuse_bonus);
		}
	}

	fn apply_floor(&mut self) {
		self.left = self.left.max(self.floor);
		self.right = self.right.max(self.floor);
	}

	fn random_side<R: Rng + ?Sized>(&mut self, rng: &mut R) -> &mut i64 {
		if rng.random::<f64>() <= 0.5 { &mut self.left } else { &mut self.right }
	}

	fn highest_side(&mut self) -> &mut i64 {
		if self.left <= self.right { &mut self.right } else { &mut self.left }
	}

	fn lowest_side(&mut self) -> &mut i64 {
		if self.left <= self.right { &mut self.left } else { &mut self.right }
	}

	pub fn add_random<R: Rng + ?Sized>(&mut self, value: i64, rng: &mut R) {
		*self.random_side(rng) += value;
		self.apply_floor();
	}

	pub fn add_highest(&mut self, value: i64) {
		*self.highest_side() += value * i64::from(self.highest_repeats);
		self.apply_floor();
	}

	pub fn add_lowest(&mut self, value: i64) {
		*self.lowest_side() += value;
		self.apply_floor();
	}

	pub fn add_all(&mut self, value: i64) {
		self.left += value;
		self.right += value;
		self.apply_floor();
	}

	pub fn double_random<R: Rng + ?Sized>(&mut self, rng: &mut R) {
		*self.random_side(rng) *= 2;
		self.apply_floor();
	}

	pub fn double_highest(&mut self) {
		*self.highest_side() *= 1 << self.highest_repeats;
		self.apply_floor();
	}

	pub fn double_lowest(&mut self) {
		*self.lowest_side() *= 2;
		self.apply_floor();
	}

	/// A forge hit: hits every colour while Charge has charges left, otherwise a random one.
	pub fn forge_hit<R: Rng + ?Sized>(&mut self, value: i64, rng: &mut R) {
		if self.stack.charge_charges != 0 {
			self.add_all(value);
			self.stack.charge_charges -= 1;
		} else {
			self.add_random(value, rng);
		}
	}

	/// Applies the accumulated Enchant and Overload debuffs: first those that hit
	/// both colours, all together, then each one that hits a random colour.
	pub fn apply_debuffs<R: Rng + ?Sized>(&mut self, rng: &mut R) {
		let debuffs = [self.stack.enchant_debuff, self.stack.overload_debuff];
		let both: i64 = debuffs.iter().filter(|d| !d.random).map(|d| d.value * d.count).sum();
		if both != 0 {
			self.debuff_both(both);
		}
		for debuff in debuffs.iter().filter(|d| d.random) {
			for _ in 0..debuff.count {
				self.debuff_random(debuff.value, rng);
			}
		}
	}

	/// Lowers both colours by `debuffs`.
	fn debuff_both(&mut self, debuffs: i64) {
		if self.stack.reversal {
			// Reversal turns debuffs into a bonus on the highest colour
			self.add_highest(debuffs);
		} else if self.stack.harmonia {
			// Harmonia only allows the highest colour to decrease
			self.add_highest(-debuffs);
		} else {
			self.add_all(-debuffs);
		}
	}

	/// Lowers a random colour by `debuff`.
	fn debuff_random<R: Rng + ?Sized>(&mut self, debuff: i64, rng: &mut R) {
		if self.stack.reversal {
			// Reversal turns debuffs into a bonus on the highest colour
			self.add_highest(debuff);
		} else if self.stack.harmonia {
			// Harmonia only allows the highest colour to decrease, so the debuff
			// lands only if it picks the highest (the right one on a tie, as in
			// `highest_side`)
			let picked_highest = if rng.random::<f64>() <= 0.5 {
				self.left > self.right
			} else {
				self.left <= self.right
			};
			if picked_highest {
				self.add_highest(-debuff);
			}
		} else {
			self.add_random(-debuff, rng);
		}
	}
}

impl fmt::Display for Score {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}x{} ({})", self.left, self.right, self.total())
	}
}
