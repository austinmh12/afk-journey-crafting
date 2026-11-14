#[derive(Debug, Clone, Copy)]
struct Score {
	pub left: u64,
	pub right: u64,
	pub stack: Stack,
}

impl Default for Score {
	fn default() -> Self {
		Self {
			left: 1,
			right: 1,
			stack: Stack::default(),
		}
	}
}

#[derive(Debug, Clone, Copy)]
struct Stack {
	// Kitchen has no modifiers
	// Forge modifiers
	pub charge_charges: u64,
	pub multi_forge_amt: u64,
	pub heat_up_bonus: u64,
	pub forge_expert_mult: u64,
	// Alchemy modifiers
	pub overcharge_amt: u64,
	pub overcharge_mult: u64,
	pub reversal: bool,
	pub reversal_mult: u64,
	pub enchant_debuff: u64,
	pub overload_debuff: u64,
	pub harmonia: bool,
	pub fuse_bonus: u64,
	pub fuse_gap: u64,
}

impl Default for Stack {
	fn default() -> Self {
		Self {
			charge_charges: 0,
			multi_forge_amt: 0,
			heat_up_bonus: 0,
			forge_expert_mult: 1,
			overcharge_amt: 0,
			overcharge_mult: 1,
			reversal: false,
			reversal_mult: 1,
			enchant_debuff: 0,
			overload_debuff: 0,
			harmonia: false,
			fuse_bonus: 0,
			fuse_gap: 0,
		}
	}
}

#[derive(Debug, Clone)]
struct Recipe {
	pub name: String,
	pub location: Location,
	pub cards: u8,
	pub stars: Vec<u64>,
}

impl Recipe {
	pub fn stars(self, score: u64) -> usize {
		todo!("Calculate return the first index the provided score is less than the value of self.stars");
	}
}

#[derive(Debug, Clone, Copy)]
enum Location {
	Kitchen,
	Alchemy,
	Forge,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Card {
	pub name: String,
	pub level: u8,
}

trait Scoreable {
	fn score<T: Scoreable>(self, score: &mut Score, recipe: String, previous: &Vec<T>);
}

fn main() {
    println!("Hello, world!");
}
