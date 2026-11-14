use rand::Rng;

use crate::card::{Card, CardKind};
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
	pub fn score<R: Rng + ?Sized>(&self, recipe: &str, tables: &CardTables, rng: &mut R) -> ScoreResults {
		let mut order = self.cards.clone();
		let mut results = ScoreResults::new(score_order(&order, recipe, tables, rng));
		while next_permutation(&mut order) {
			results.push(score_order(&order, recipe, tables, rng));
		}
		results
	}
}

/// Plays the cards in `order` and returns the final score.
pub fn score_order<R: Rng + ?Sized>(order: &[Card], recipe: &str, tables: &CardTables, rng: &mut R) -> Score {
	let mut score = Score::default();
	for (i, card) in order.iter().enumerate() {
		let context = match card.kind {
			CardKind::ExactCook => order,
			CardKind::Sharpening => &order[i + 1..],
			_ => &order[..i],
		};
		card.score(&mut score, recipe, context, tables, rng);
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
