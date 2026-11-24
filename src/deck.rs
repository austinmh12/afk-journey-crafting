use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::card::{Card, CardKind};
use crate::recipe::Recipe;
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

/// Whether a deck with `average` at position `index` in [`generate_decks`]'s output
/// beats the current best. Highest average wins; ties go to the earlier deck, so the
/// winner doesn't depend on the order decks were scored in.
pub fn beats(average: i64, index: usize, best_average: i64, best_index: usize) -> bool {
	average > best_average || (average == best_average && index < best_index)
}

/// An unordered set of cards to craft with. Cards are kept sorted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deck {
	pub cards: Vec<Card>,
}

impl Deck {
	/// Scores every distinct play order of this deck.
	pub fn score<R: Rng + ?Sized>(&self, recipe: &Recipe, tables: &CardTables, rng: &mut R) -> ScoreResults {
		let mut results: Option<ScoreResults> = None;
		for_each_order(&self.cards, 1, recipe, tables, rng, |_, score| match &mut results {
			Some(results) => results.push(score),
			None => results = Some(ScoreResults::new(score)),
		});
		results.expect("a deck has at least one play order")
	}

	/// Scores every distinct play order of this deck and gathers stats about them,
	/// including the crafts that scored lowest and highest.
	///
	/// Random effects make one craft of a play order a single sample of what it
	/// can score. To keep that noise about the same for every deck, each order is
	/// scored enough times for about `samples` crafts in all, and at least once.
	pub fn stats<R: Rng + ?Sized>(
		&self,
		recipe: &Recipe,
		tables: &CardTables,
		samples: u64,
		rng: &mut R,
	) -> DeckStats {
		let mut cards = self.cards.clone();
		cards.sort();
		let orders = distinct_orders(&cards);
		let rolls = rolls_per_order(orders, samples);
		let mut results: Option<ScoreResults> = None;
		let mut minimum = Played::default();
		let mut maximum = Played::default();
		let mut star_orders = vec![0; recipe.stars.len()];
		for_each_order(&cards, rolls, recipe, tables, rng, |order, score| {
			match &mut results {
				Some(results) => {
					// Strict comparisons keep the first craft seen on ties, like ScoreResults
					if score.total() < results.minimum.total() {
						minimum = Played::new(order, score);
					}
					if score.total() > results.maximum.total() {
						maximum = Played::new(order, score);
					}
					results.push(score);
				}
				None => {
					results = Some(ScoreResults::new(score));
					minimum = Played::new(order, score);
					maximum = minimum.clone();
				}
			}
			for count in &mut star_orders[..recipe.stars(score.total())] {
				*count += 1;
			}
		});
		let results = results.expect("a deck has at least one play order");
		debug_assert_eq!(results.count as u64, orders * rolls);
		DeckStats {
			orders,
			rolls,
			average: results.average(),
			minimum,
			maximum,
			star_orders,
			cards,
		}
	}
}

/// One play order of a deck and the score it got.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Played {
	pub order: Vec<Card>,
	pub left: i64,
	pub right: i64,
}

impl Played {
	fn new(order: &[Card], score: Score) -> Self {
		Self {
			order: order.to_vec(),
			left: score.left,
			right: score.right,
		}
	}

	pub fn total(&self) -> i64 {
		self.left * self.right
	}
}

/// Stats over every distinct play order of one deck. See [`Deck::stats`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeckStats {
	/// The deck's cards, sorted.
	pub cards: Vec<Card>,
	/// How many distinct play orders were scored.
	pub orders: u64,
	/// How many times each play order was scored. Decks scored before this was
	/// added were scored once per order.
	#[serde(default = "one")]
	pub rolls: u64,
	/// Average score over every craft.
	pub average: i64,
	/// The lowest-scoring craft (the first one, on ties).
	pub minimum: Played,
	/// The highest-scoring craft (the first one, on ties).
	pub maximum: Played,
	/// How many crafts earned at least each star, lowest star first.
	pub star_orders: Vec<u64>,
}

fn one() -> u64 {
	1
}

impl DeckStats {
	/// How many crafts were scored: every play order, [`Self::rolls`] times each.
	pub fn crafts(&self) -> u64 {
		self.orders * self.rolls
	}

	/// How many crafts earned exactly 0, 1, 2… stars, up to every star, so they
	/// add up to [`Self::crafts`].
	pub fn star_counts(&self) -> Vec<u64> {
		// `star_orders` counts crafts that earned at least each star
		let at_least = |stars: usize| match stars {
			0 => self.crafts(),
			_ => self.star_orders.get(stars - 1).copied().unwrap_or(0),
		};
		(0..=self.star_orders.len())
			.map(|stars| at_least(stars).saturating_sub(at_least(stars + 1)))
			.collect()
	}
}

/// How many distinct play orders `cards` has, given sorted: the number of
/// permutations, divided by the orderings of each card's copies among themselves.
pub fn distinct_orders(cards: &[Card]) -> u64 {
	let factorial = |n: usize| (1..=n as u64).product::<u64>();
	let mut orders = factorial(cards.len());
	for copies in cards.chunk_by(|a, b| a == b) {
		orders /= factorial(copies.len());
	}
	orders
}

/// How many times to score each of `orders` play orders for about `samples`
/// crafts in all: rounded up, and at least once.
pub fn rolls_per_order(orders: u64, samples: u64) -> u64 {
	samples.div_ceil(orders.max(1)).max(1)
}

/// Scores every distinct play order of `cards` `rolls` times, and calls `f` with
/// each order and its score. `cards` must be sorted to start from the first order.
fn for_each_order<R: Rng + ?Sized>(
	cards: &[Card],
	rolls: u64,
	recipe: &Recipe,
	tables: &CardTables,
	rng: &mut R,
	mut f: impl FnMut(&[Card], Score),
) {
	let mut order = cards.to_vec();
	loop {
		for _ in 0..rolls {
			let score = score_order(&order, recipe, tables, rng);
			f(&order, score);
		}
		if !next_permutation(&mut order) {
			break;
		}
	}
}

/// Replaces each Banquet with the card it flips (the card before it), and returns
/// how many times each card is flipped. Banquet after Banquet flips the same card again.
fn resolve_banquets(order: &[Card], tables: &CardTables) -> (Vec<Card>, Vec<u32>) {
	let mut cards: Vec<Card> = Vec::with_capacity(order.len());
	let mut flips: Vec<u32> = Vec::with_capacity(order.len());
	for &card in order {
		match (card.kind, cards.last()) {
			(CardKind::Banquet, Some(&last)) => {
				cards.push(last);
				flips.push(tables.banquet.get(card.level));
			}
			_ => {
				cards.push(card);
				flips.push(1);
			}
		}
	}
	(cards, flips)
}

/// Plays the cards in `order` and returns the final score.
pub fn score_order<R: Rng + ?Sized>(order: &[Card], recipe: &Recipe, tables: &CardTables, rng: &mut R) -> Score {
	let mut score = recipe.start_score(rng);
	// Other cards see a Banquet as the card it flipped (e.g. it continues a Cut streak).
	// Most play orders have no Banquet, so only build the swapped order when needed.
	let resolved;
	let (cards, flips): (&[Card], &[u32]) = if order.iter().any(|c| c.kind == CardKind::Banquet) {
		resolved = resolve_banquets(order, tables);
		(&resolved.0, &resolved.1)
	} else {
		(order, &[])
	};
	for (i, &card) in cards.iter().enumerate() {
		// Without Banquets every card flips once
		let flips = flips.get(i).copied().unwrap_or(1);
		let context = match card.kind {
			CardKind::ExactCook => cards,
			CardKind::Sharpening => &cards[i + 1..],
			_ => &cards[..i],
		};
		for _ in 0..flips * recipe.triggers(card.kind) {
			card.score(&mut score, &recipe.name, context, tables, rng);
		}
		recipe.apply_flip_power(&mut score);
	}
	score.finalize(rng);
	recipe.apply_end_power(&mut score);
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
	use crate::recipe::{Location, powers};

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
		// Reforge lvl 1 adds 3 to both colours: 1 + 3 + 3 = 7
		assert_eq!(total(&[CardKind::Reforge, CardKind::Banquet]), 7 * 7);
		// A second Banquet flips the same card again
		assert_eq!(total(&[CardKind::Reforge, CardKind::Banquet, CardKind::Banquet]), 10 * 10);
		// Nothing to flip when played first
		assert_eq!(total(&[CardKind::Banquet, CardKind::Reforge]), 4 * 4);
	}

	/// Scores cards given as (kind, level) and returns left + right, which doesn't
	/// depend on which colour random hits land on. Both colours start at 1.
	fn points(order: &[(CardKind, u8)]) -> i64 {
		let score = play(Location::Forge, order);
		score.left + score.right - 2
	}

	#[test]
	fn dried_mushroom_rewards_seven_heat_control_triggers() {
		use CardKind::*;
		let check = |order: &[(CardKind, u8)]| {
			let score = play_recipe(powers::DRIED_MUSHROOM, Location::Kitchen, order);
			let triggers = i64::from(score.heat_control_triggers);
			// Heat Control lvl 1 adds 3 per trigger, then +3 to both colours at 7+
			let bonus = if triggers >= 7 { 3 * 2 } else { 0 };
			assert_eq!(score.left + score.right - 2, 3 * triggers + bonus);
			triggers
		};
		// 3 Ferment lvl 2 give 6 extra triggers: always at least 7
		assert!(check(&[(Ferment, 2), (Ferment, 2), (Ferment, 2), (HeatControl, 1)]) >= 7);
		// Retriggers alone may or may not reach 7
		check(&[(HeatControl, 1)]);
	}

	#[test]
	fn odd_sweet_rewards_small_final_gap() {
		use CardKind::*;
		// Slow Cook lvl 2 adds 2 to both: 3x3, gap 0, so both +5
		assert_eq!(play_recipe(powers::ODD_SWEET, Location::Kitchen, &[(SlowCook, 2)]).total(), 8 * 8);
		// Overload lvl 2 makes it 1x101: gap too big
		assert_eq!(play_recipe(powers::ODD_SWEET, Location::Kitchen, &[(Overload, 2)]).total(), 101);
	}

	#[test]
	fn healthy_candied_skewer_lowers_highest_colour() {
		use CardKind::*;
		// Overload lvl 2 makes it 1x101, then the 101 drops to 1
		let score = play_recipe(powers::HEALTHY_CANDIED_SKEWER, Location::Kitchen, &[(Overload, 2)]);
		assert_eq!((score.left, score.right), (1, 1));
	}

	/// Scores a Forge recipe with a power and returns left + right minus the starting 2.
	fn forge_points(recipe: &str, order: &[(CardKind, u8)]) -> i64 {
		let score = play_recipe(recipe, Location::Forge, order);
		score.left + score.right - 2
	}

	#[test]
	fn protective_pick_hammer_flips_enhanced_cards_again() {
		use CardKind::*;
		let pph = |order: &[(CardKind, u8)]| forge_points(powers::PROTECTIVE_PICK_HAMMER, order);
		// Heat Up lvl 4 (+12) and Charge lvl 3 (every hit hits both colours) are 2 boosts
		// Forge lvl 6: flips twice for 15 + 12 on both colours
		assert_eq!(pph(&[(HeatUp, 4), (Charge, 3), (Forge, 6)]), 27 * 2 * 2);
		assert_eq!(points(&[(HeatUp, 4), (Charge, 3), (Forge, 6)]), 27 * 2);
		// Forge Expert lvl 5: value 5 + 12, bonus 5 growing with each hit
		assert_eq!(pph(&[(HeatUp, 4), (Charge, 3), (ForgeExpert, 5)]), (17 + 22) * 2);
		assert_eq!(points(&[(HeatUp, 4), (Charge, 3), (ForgeExpert, 5)]), 17 * 2);
		// Only 1 boost: no second flip
		assert_eq!(pph(&[(Charge, 3), (Forge, 6)]), 15 * 2);
		// Pyroburst lvl 1 (12 + 12) hits 3 times per flip: 6 in total
		assert_eq!(pph(&[(HeatUp, 4), (Charge, 3), (Pyroburst, 1)]), 24 * 6 * 2);
		assert_eq!(points(&[(HeatUp, 4), (Charge, 3), (Pyroburst, 1)]), 24 * 3 * 2);
		// Charge lvl 1 has 1 charge: the first flip uses it (both colours, then 2 extra
		// single hits), so the second flip has only Heat Up and doesn't get Pyroburst's extras
		assert_eq!(pph(&[(HeatUp, 4), (Charge, 1), (Pyroburst, 1)]), 24 * 2 + 24 * 2 + 24);
		// Soulbind lvl 1 is covered too: 12 from Heat Up on both colours, twice
		assert_eq!(pph(&[(HeatUp, 4), (Charge, 3), (Soulbind, 1)]), 12 * 2 * 2);
		assert_eq!(points(&[(HeatUp, 4), (Charge, 3), (Soulbind, 1)]), 12 * 2);
		// Each Soulbind flip refreshes Multi-Forge lvl 1 to 2 queued hits: 3 hits per flip
		assert_eq!(pph(&[(MultiForge, 1), (HeatUp, 4), (Soulbind, 1)]), 12 * 3 * 2);
	}

	#[test]
	fn forge_expert_may_trigger_again_in_stewpot_and_firefang() {
		use CardKind::*;
		for recipe in [powers::COPPER_STEWPOT, powers::FIREFANG_SWORD] {
			// Forge Expert lvl 5 scores 5, or 5 + 10 with the extra trigger (30%)
			let results: Vec<i64> = (0..200).map(|_| forge_points(recipe, &[(ForgeExpert, 5)])).collect();
			assert!(results.iter().all(|&p| p == 5 || p == 15));
			assert!(results.contains(&5) && results.contains(&15));
		}
		assert_eq!(points(&[(ForgeExpert, 5)]), 5);
	}

	#[test]
	fn first_forge_cards_hit_all_colours() {
		use CardKind::*;
		// Forge lvl 6 scores 15: on both colours, or on one
		assert_eq!(forge_points(powers::CARVED_BOX, &[(Forge, 6), (Forge, 6)]), 30 + 15);
		assert_eq!(forge_points(powers::FIREPROOF_HELM, &[(Forge, 6); 4]), 30 * 3 + 15);
		assert_eq!(points(&[(Forge, 6), (Forge, 6)]), 15 + 15);
		// The power doesn't use up Charge lvl 1's single charge; the second Forge does
		assert_eq!(forge_points(powers::CARVED_BOX, &[(Charge, 1), (Forge, 6), (Forge, 6)]), 30 + 30);
	}

	#[test]
	fn warm_stone_armor_rewards_six_artisan_triggers() {
		use CardKind::*;
		// Multi-Forge lvl 2 queues 3 hits: Forge lvl 6 triggers 4 times, no bonus
		let score = play_recipe(powers::WARM_STONE_ARMOR, Location::Forge, &[(MultiForge, 2), (Forge, 6)]);
		assert_eq!(score.artisan_triggers, 4);
		assert_eq!(score.left + score.right - 2, 15 * 4);
		// Two Multi-Forges: 7 triggers, so both colours +3
		let order = [(MultiForge, 2), (MultiForge, 2), (Forge, 6)];
		let score = play_recipe(powers::WARM_STONE_ARMOR, Location::Forge, &order);
		assert_eq!(score.artisan_triggers, 7);
		assert_eq!(score.left + score.right - 2, 15 * 7 + 3 * 2);
	}

	/// Scores an Alchemy recipe and returns its colours as (lowest, highest).
	fn alchemy(recipe: &str, order: &[(CardKind, u8)]) -> (i64, i64) {
		let score = play_recipe(recipe, Location::Alchemy, order);
		(score.left.min(score.right), score.left.max(score.right))
	}

	#[test]
	fn alchemy_flip_powers() {
		use CardKind::*;
		// Distill doubles the highest colour, then the power applies
		// +3 to highest: 1x1 -> 1x2 -> 1x5 -> 1x10 -> 1x13
		assert_eq!(alchemy(powers::CALMING_WARMDUST, &[(Distill, 1), (Distill, 1)]), (1, 13));
		assert_eq!(alchemy(powers::SOOTHING_TONIC, &[(Distill, 1), (Distill, 1)]), (1, 13));
		// +1 to lowest: 1x1 -> 1x2 -> 2x2 -> 2x4 -> 3x4
		assert_eq!(alchemy(powers::WARMDUST, &[(Distill, 1), (Distill, 1)]), (3, 4));
		assert_eq!(alchemy(powers::ILLUSION_POTION, &[(Distill, 1), (Distill, 1)]), (3, 4));
	}

	#[test]
	fn alchemy_extra_trigger_powers() {
		use CardKind::*;
		// Grind lvl 8: -7 to highest (floored at 1), then +35 to lowest; twice
		assert_eq!(alchemy(powers::WARMING_INCENSE, &[(Grind, 8)]), (29, 36));
		assert_eq!(alchemy("", &[(Grind, 8)]), (1, 36));
		// Ingredients lvl 6: +30 to highest, -5 to lowest (floored at 1); twice
		assert_eq!(alchemy(powers::CALMWIND_INCENSE, &[(Ingredients, 6)]), (1, 61));
		assert_eq!(alchemy("", &[(Ingredients, 6)]), (1, 31));
	}

	#[test]
	fn alchemy_start_and_end_powers() {
		use CardKind::*;
		// Fireward Ring: +15 to a random colour before any card
		assert_eq!(alchemy(powers::FIREWARD_RING, &[]), (1, 16));
		// Earth Antidote: effects on the highest colour apply twice
		assert_eq!(alchemy(powers::EARTH_ANTIDOTE, &[(Overload, 2)]), (1, 201));
		assert_eq!(alchemy(powers::EARTH_ANTIDOTE, &[(Distill, 1)]), (1, 4));
		assert_eq!(alchemy(powers::EARTH_ANTIDOTE, &[(Ingredients, 6)]), (1, 61));
		// Frost Antidote: 1x101 has highest > 3x lowest, so the lowest doubles
		assert_eq!(alchemy(powers::FROST_ANTIDOTE, &[(Overload, 2)]), (2, 101));
		assert_eq!(alchemy(powers::FROST_ANTIDOTE, &[(Distill, 1)]), (1, 2));
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
	fn deck_stats_cover_every_play_order() {
		use CardKind::*;
		// Reforge lvl 1 adds 3 to both colours; Banquet flips the card before it again.
		// Orders, in sorted order: B R R -> 7x7, R B R -> 10x10, R R B -> 10x10
		let recipe = Recipe {
			name: String::new(),
			location: Location::Forge,
			cards: 3,
			stars: vec![40, 100, 300],
		};
		// Unsorted on purpose: stats sorts the deck to cover every order
		let deck = Deck { cards: vec![card(Reforge), card(Banquet), card(Reforge)] };
		// A budget of 1 scores each order once
		let stats = deck.stats(&recipe, &tables(), 1, &mut rand::rng());
		assert_eq!(stats.rolls, 1);
		assert_eq!(stats.cards, vec![card(Banquet), card(Reforge), card(Reforge)]);
		assert_eq!(stats.orders, 3);
		assert_eq!(stats.average, (49 + 100 + 100) / 3);
		assert_eq!(stats.minimum.order, vec![card(Banquet), card(Reforge), card(Reforge)]);
		assert_eq!(stats.minimum.total(), 49);
		// The first of the two 100s
		assert_eq!(stats.maximum.order, vec![card(Reforge), card(Banquet), card(Reforge)]);
		assert_eq!(stats.maximum.total(), 100);
		assert_eq!(stats.star_orders, vec![3, 2, 0]);
		// Exactly 0, 1, 2 and 3 stars: one order at 49 (1 star), two at 100 (2 stars)
		assert_eq!(stats.star_counts(), vec![0, 1, 2, 0]);
		// No random effects, so more rolls give the same stats, with every count 10 times over
		let sampled = deck.stats(&recipe, &tables(), 30, &mut rand::rng());
		assert_eq!((sampled.orders, sampled.rolls, sampled.crafts()), (3, 10, 30));
		assert_eq!(sampled.average, stats.average);
		assert_eq!(sampled.star_counts(), vec![0, 10, 20, 0]);
		assert_eq!((sampled.minimum.clone(), sampled.maximum.clone()), (stats.minimum.clone(), stats.maximum.clone()));
		// Matches the calculator's scoring of the same deck
		let sorted = Deck { cards: stats.cards.clone() };
		assert_eq!(sorted.score(&recipe, &tables(), &mut rand::rng()).average(), stats.average);
	}

	#[test]
	fn counting_orders_and_rolls() {
		use CardKind::*;
		let cards = |kinds: &[CardKind]| kinds.iter().map(|&kind| card(kind)).collect::<Vec<_>>();
		assert_eq!(distinct_orders(&cards(&[Cut, Cut, Season])), 3);
		assert_eq!(distinct_orders(&cards(&[Cut; 5])), 1);
		// 8! / (2! 3!)
		assert_eq!(distinct_orders(&cards(&[Cut, Cut, Season, Bake, Bake, Bake, Ferment, Banquet])), 3360);
		assert_eq!(distinct_orders(&cards(&[Cut, Season, Bake, Ferment, Banquet, SlowCook, HeatControl, ExactCook, Forge])), 362_880);
		assert_eq!(rolls_per_order(1, 50_000), 50_000);
		// Rounded up
		assert_eq!(rolls_per_order(3360, 50_000), 15);
		// Every order at least once, even past the budget
		assert_eq!(rolls_per_order(362_880, 50_000), 1);
		assert_eq!(rolls_per_order(3, 0), 1);
	}

	#[test]
	fn stats_saved_before_rolls_were_scored_once_per_order() {
		let saved = "(cards: [], orders: 6, average: 10, minimum: (order: [], left: 1, right: 1), 			maximum: (order: [], left: 1, right: 1), star_orders: [4, 1])";
		let stats: DeckStats = ron::from_str(saved).unwrap();
		assert_eq!((stats.rolls, stats.crafts()), (1, 6));
		assert_eq!(stats.star_counts(), vec![2, 3, 1]);
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
