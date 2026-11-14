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
	pub enchant_debuff: i64,
	pub overload_debuff: i64,
	pub harmonia: bool,
	pub fuse_bonus: i64,
	pub fuse_gap: i64,
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

	/// Applies the end-of-craft effects. Call once after the last card is scored.
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
		*self.highest_side() += value;
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
		*self.highest_side() *= 2;
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

	/// Applies the accumulated Enchant and Overload debuffs.
	pub fn apply_debuffs(&mut self) {
		let debuffs = self.stack.enchant_debuff + self.stack.overload_debuff;
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
}

impl fmt::Display for Score {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}x{} ({})", self.left, self.right, self.total())
	}
}
