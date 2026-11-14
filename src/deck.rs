use rand::Rng;

use crate::card::{Card, CardKind};
use crate::recipe::{Location, Recipe};
use crate::score::Score;
use crate::tables::CardTables;

/// Min, max and average of a set of scored crafts.
#[derive(Debug, Clone, Copy)]
pub struct ScoreResults {
	pub minimum: Score,
	pub maximum: Score,
	sum: i64,
	count: i64,
}

impl ScoreResults {
	fn new(first: Score) -> Self {
		Self {
			minimum: first,
			maximum: first,
			sum: first.total(),
			count: 1,
		}
	}

	fn push(&mut self, score: Score) {
		// Strict comparisons keep the first score seen on ties
		if score.total() < self.minimum.total() {
			self.minimum = score;
		}
		if score.total() > self.maximum.total() {
			self.maximum = score;
		}
		self.sum += score.total();
		self.count += 1;
	}

	pub fn average(&self) -> i64 {
		self.sum.div_euclid(self.count)
	}
}

/// An unordered set of cards to craft with. Cards are kept sorted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deck {
	pub cards: Vec<Card>,
}

impl Deck {
	/// Scores every distinct play order of this deck.
	pub fn score<R: Rng + ?Sized>(&self, recipe: &Recipe, tables: &CardTables, rng: &mut R) -> ScoreResults {
		let mut order = self.cards.clone();
		let mut results = ScoreResults::new(score_order(&order, recipe, tables, rng));
		while next_permutation(&mut order) {
			results.push(score_order(&order, recipe, tables, rng));
		}
		results
	}
}

/// Replaces each Banquet with the card it flips (the card before it), paired with
/// how many times that card is flipped. Banquet after Banquet flips the same card again.
fn resolve_banquets(order: &[Card], tables: &CardTables) -> Vec<(Card, u32)> {
	let mut played: Vec<(Card, u32)> = Vec::with_capacity(order.len());
	for &card in order {
		let flip = match (card.kind, played.last()) {
			(CardKind::Banquet, Some(&(last, _))) => (last, tables.banquet.get(card.level)),
			_ => (card, 1),
		};
		played.push(flip);
	}
	played
}

/// Plays the cards in `order` and returns the final score.
pub fn score_order<R: Rng + ?Sized>(order: &[Card], recipe: &Recipe, tables: &CardTables, rng: &mut R) -> Score {
	let mut score = Score::default();
	if recipe.location == Location::Alchemy {
		// Alchemy colours can never drop below 1
		score.floor = 1;
	}
	let played = resolve_banquets(order, tables);
	// Other cards see a Banquet as the card it flipped (e.g. it continues a Cut streak)
	let cards: Vec<Card> = played.iter().map(|&(card, _)| card).collect();
	for (i, &(card, flips)) in played.iter().enumerate() {
		let context = match card.kind {
			CardKind::ExactCook => &cards[..],
			CardKind::Sharpening => &cards[i + 1..],
			_ => &cards[..i],
		};
		for _ in 0..flips {
			card.score(&mut score, &recipe.name, context, tables, rng);
		}
	}
	score.finalize(rng);
	score
}

/// Generates every distinct deck of `size` cards from `available`, where each
/// entry is a card and how many copies of it may be used.
pub fn generate_decks(available: &[(Card, usize)], size: usize) -> Vec<Deck> {
	let mut remaining: Vec<usize> = available.iter().map(|(_, count)| *count).collect();
	let mut current = Vec::with_capacity(size);
	let mut decks = Vec::new();
	build_decks(available, &mut remaining, 0, size, &mut current, &mut decks);
	decks
}

fn build_decks(
	available: &[(Card, usize)],
	remaining: &mut [usize],
	start: usize,
	size: usize,
	current: &mut Vec<Card>,
	decks: &mut Vec<Deck>,
) {
	if current.len() == size {
		let mut cards = current.clone();
		cards.sort();
		decks.push(Deck { cards });
		return;
	}
	// Only pick from `start` onward so each set of cards is produced once
	for i in start..available.len() {
		if remaining[i] == 0 {
			continue;
		}
		remaining[i] -= 1;
		current.push(available[i].0);
		build_decks(available, remaining, i, size, current, decks);
		current.pop();
		remaining[i] += 1;
	}
}

/// Rearranges `v` into the next lexicographic permutation, skipping duplicate
/// orderings. Returns false once `v` was the last permutation.
fn next_permutation<T: Ord>(v: &mut [T]) -> bool {
	if v.len() < 2 {
		return false;
	}
	let mut i = v.len() - 1;
	while i > 0 && v[i - 1] >= v[i] {
		i -= 1;
	}
	if i == 0 {
		return false;
	}
	let mut j = v.len() - 1;
	while v[j] <= v[i - 1] {
		j -= 1;
	}
	v.swap(i - 1, j);
	v[i..].reverse();
	true
}

#[cfg(test)]
mod tests {
	use super::*;

	fn card(kind: CardKind) -> Card {
		Card { kind, level: 1 }
	}

	fn tables() -> CardTables {
		ron::from_str(include_str!("../data/cards.ron")).expect("cards.ron should parse")
	}

	/// Scores cards given as (kind, level) for a recipe at `location`.
	fn play(location: Location, order: &[(CardKind, u8)]) -> Score {
		play_recipe("", location, order)
	}

	fn play_recipe(name: &str, location: Location, order: &[(CardKind, u8)]) -> Score {
		let recipe = Recipe { name: name.to_string(), location, cards: order.len(), stars: vec![] };
		let order: Vec<Card> = order.iter().map(|&(kind, level)| Card { kind, level }).collect();
		score_order(&order, &recipe, &tables(), &mut rand::rng())
	}

	fn total(order: &[CardKind]) -> i64 {
		let order: Vec<(CardKind, u8)> = order.iter().map(|&kind| (kind, 1)).collect();
		play(Location::Forge, &order).total()
	}

	#[test]
	fn banquet_flips_previous_card() {
		// Reforge lvl 1 adds 5 to both colours: 1 + 5 + 5 = 11
		assert_eq!(total(&[CardKind::Reforge, CardKind::Banquet]), 11 * 11);
		// A second Banquet flips the same card again
		assert_eq!(total(&[CardKind::Reforge, CardKind::Banquet, CardKind::Banquet]), 16 * 16);
		// Nothing to flip when played first
		assert_eq!(total(&[CardKind::Banquet, CardKind::Reforge]), 6 * 6);
	}

	/// Scores cards given as (kind, level) and returns left + right, which doesn't
	/// depend on which colour random hits land on. Both colours start at 1.
	fn points(order: &[(CardKind, u8)]) -> i64 {
		let score = play(Location::Forge, order);
		score.left + score.right - 2
	}

	#[test]
	fn protective_pick_hammer_repeats_artisan_cards() {
		use CardKind::*;
		let pph = |order: &[(CardKind, u8)]| {
			let score = play_recipe("Protective Pick Hammer", Location::Forge, order);
			score.left + score.right - 2
		};
		// Heat Up lvl 4 (+12) and Charge lvl 3 (every hit hits both colours) are 2 boosts
		// Forge lvl 6: 3 hits of 15 + 12 on both colours
		assert_eq!(pph(&[(HeatUp, 4), (Charge, 3), (Forge, 6)]), 27 * 3 * 2);
		assert_eq!(points(&[(HeatUp, 4), (Charge, 3), (Forge, 6)]), 27 * 2);
		// Forge Expert lvl 5: value 5 + 12, bonus 5 growing with each of the 3 hits
		assert_eq!(pph(&[(HeatUp, 4), (Charge, 3), (ForgeExpert, 5)]), (17 + 22 + 27) * 2);
		assert_eq!(points(&[(HeatUp, 4), (Charge, 3), (ForgeExpert, 5)]), 17 * 2);
		// Only 1 boost: no repeat
		assert_eq!(pph(&[(Charge, 3), (Forge, 6)]), 15 * 2);
		// Pyroburst lvl 1 (12 + 12) repeats in every recipe
		assert_eq!(points(&[(HeatUp, 4), (Charge, 3), (Pyroburst, 1)]), 24 * 3 * 2);
	}

	#[test]
	fn alchemy_colours_never_drop_below_one() {
		// Ingredients lvl 6: +30 to the highest, -5 to the lowest (1 - 5 floors at 1)
		let score = play(Location::Alchemy, &[(CardKind::Ingredients, 6)]);
		assert_eq!((score.left, score.right), (1, 31));
		// Other locations have no floor
		let score = play(Location::Kitchen, &[(CardKind::Ingredients, 6)]);
		assert_eq!((score.left, score.right), (-4, 31));
	}

	#[test]
	fn fuse_bonus_needs_small_gap() {
		use CardKind::*;
		// Fuse lvl 3 (bonus 20, gap 50): ends at 1x1, gap 0 < 50, so both colours +20
		assert_eq!(play(Location::Alchemy, &[(Fuse, 3)]).total(), 21 * 21);
		// Fuse lvl 1 (bonus 5, gap 10): Overload lvl 2 makes it 1x101, gap 100, no bonus
		assert_eq!(play(Location::Alchemy, &[(Fuse, 1), (Overload, 2)]).total(), 101);
	}

	#[test]
	fn soulbind_uses_earlier_enhancements() {
		use CardKind::*;
		// Flipped first: lvl 1 base value is 0
		assert_eq!(points(&[(Soulbind, 1)]), 0);
		// Heat Up lvl 4 sets the bonus to 12, Soulbind scores 12, Forge lvl 6 scores 15 + 12
		assert_eq!(points(&[(HeatUp, 4), (Soulbind, 1), (Forge, 6)]), 12 + 27);
		// Two Heat Ups stack to 24
		assert_eq!(points(&[(HeatUp, 4), (HeatUp, 4), (Soulbind, 1)]), 24);
		// Forge Expert lvl 5 scores 5 (mult 0) and raises the mult to 1; Soulbind gets 5 * 1
		assert_eq!(points(&[(ForgeExpert, 5), (Soulbind, 1)]), 5 + 5);
		// Soulbind doesn't raise the mult: the next Forge Expert still gets 5 + 5 * 1
		assert_eq!(points(&[(ForgeExpert, 5), (Soulbind, 1), (ForgeExpert, 5)]), 5 + 5 + 10);
		// Charge makes Soulbind lvl 2 (base 5) hit both colours
		assert_eq!(points(&[(Charge, 3), (Soulbind, 2)]), 10);
		// Multi-Forge lvl 1 queues 2 extra hits for Soulbind, which uses them up
		assert_eq!(points(&[(MultiForge, 1), (Soulbind, 2), (Forge, 1)]), 5 * 3 + 1);
		// Forge uses up both Multi-Forges (4 extra hits); Soulbind refreshes them to 4
		assert_eq!(points(&[(MultiForge, 1), (MultiForge, 1), (Forge, 6), (Soulbind, 2)]), 15 * 5 + 5 * 5);
		// Forge uses up Charge lvl 1's single charge; Soulbind refreshes it and hits both colours
		assert_eq!(points(&[(Charge, 1), (Forge, 1), (Soulbind, 2)]), 2 + 10);
	}

	#[test]
	fn soulbind_matches_in_game_examples() {
		use CardKind::*;
		// Multi-Forge lvl 2 queues 3 hits; Forge Expert lvl 5 is value 5, bonus 5
		assert_eq!(points(&[(MultiForge, 2), (ForgeExpert, 5), (Soulbind, 1)]), 130);
		assert_eq!(points(&[(ForgeExpert, 5), (MultiForge, 2), (Soulbind, 1)]), 25);
	}

	#[test]
	fn permutations_skip_duplicates() {
		let mut v = vec![1, 1, 2, 3];
		v.sort();
		let mut count = 1;
		while next_permutation(&mut v) {
			count += 1;
		}
		// 4! / 2! = 12
		assert_eq!(count, 12);
	}

	#[test]
	fn decks_are_unique_and_respect_limits() {
		let available = [(card(CardKind::Forge), 2), (card(CardKind::Charge), 1), (card(CardKind::HeatUp), 1)];
		let decks = generate_decks(&available, 3);
		// {F,F,C} {F,F,H} {F,C,H}
		assert_eq!(decks.len(), 3);
		for (i, a) in decks.iter().enumerate() {
			assert!(decks[i + 1..].iter().all(|b| a != b));
		}
	}
}
