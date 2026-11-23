//! The web page's deck builder: a recipe and the cards picked for it, one slot
//! per card the recipe needs. Kept apart from the page so it can be tested natively.

use crate::card::{Card, CardKind};
use crate::deck::DeckStats;

/// Where a dragged card comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragSource {
	/// A card from the location's available cards.
	Inventory(CardKind),
	/// The card in a deck slot.
	Slot(usize),
}

/// Where a dragged card is dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropTarget {
	Slot(usize),
	/// Back to the available cards, which takes it out of the deck.
	Inventory,
}

/// Stats asked for a deck, and the result once it arrives.
#[derive(Debug, Clone, PartialEq)]
pub struct Stats {
	pub recipe: usize,
	/// The deck's cards, sorted.
	pub cards: Vec<Card>,
	/// `None` while the deck is being scored.
	pub result: Option<DeckStats>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Builder {
	/// Index into the recipes.
	pub recipe: Option<usize>,
	/// One per card the recipe needs: `None` is an empty slot.
	pub slots: Vec<Option<CardKind>>,
	/// The latest stats asked for. Only shown while they match the deck.
	pub stats: Option<Stats>,
}

/// How many decks the history keeps for each recipe.
pub const HISTORY_LIMIT: usize = 50;

/// How many copies of `kind` may go in a deck, from the location's available cards.
pub fn limit(available: &[(Card, usize)], kind: CardKind) -> usize {
	available.iter().find(|(card, _)| card.kind == kind).map_or(0, |&(_, limit)| limit)
}

/// Each count's share of the total in tenths of a percent, rounded so the shares
/// add up to exactly 100%: the largest remainders round up. All zeros when the
/// counts are.
pub fn tenths_of_percent(counts: &[u64]) -> Vec<u64> {
	let total: u64 = counts.iter().sum();
	if total == 0 {
		return vec![0; counts.len()];
	}
	let exact: Vec<(u64, u64)> = counts.iter().map(|&count| ((count * 1000) / total, (count * 1000) % total)).collect();
	let mut shares: Vec<u64> = exact.iter().map(|&(share, _)| share).collect();
	let short = 1000 - shares.iter().sum::<u64>();
	let mut by_remainder: Vec<usize> = (0..counts.len()).collect();
	// Largest remainder first; ties go to the earlier count
	by_remainder.sort_by_key(|&i| std::cmp::Reverse(exact[i].1));
	for &i in by_remainder.iter().take(short as usize) {
		shares[i] += 1;
	}
	shares
}

/// Adds a scored deck to the front of a recipe's history, newest first. A deck
/// that was scored before is moved to the front with its new stats.
pub fn record(history: &mut Vec<DeckStats>, stats: DeckStats) {
	history.retain(|entry| entry.cards != stats.cards);
	history.insert(0, stats);
	history.truncate(HISTORY_LIMIT);
}

impl Builder {
	/// Selects a recipe that needs `size` cards. Cards already placed stay in their
	/// slots; any past the new size are taken out.
	pub fn set_recipe(&mut self, recipe: Option<usize>, size: usize) {
		self.recipe = recipe;
		self.slots.resize(if recipe.is_some() { size } else { 0 }, None);
	}

	/// How many copies of `kind` are in the deck.
	pub fn count(&self, kind: CardKind) -> usize {
		self.slots.iter().filter(|&&slot| slot == Some(kind)).count()
	}

	/// How many more copies of `kind` may go in the deck.
	pub fn remaining(&self, available: &[(Card, usize)], kind: CardKind) -> usize {
		limit(available, kind).saturating_sub(self.count(kind))
	}

	pub fn is_full(&self) -> bool {
		!self.slots.is_empty() && self.slots.iter().all(Option::is_some)
	}

	/// Puts a copy of `kind` in the first empty slot, if there is one and a copy left.
	pub fn add(&mut self, available: &[(Card, usize)], kind: CardKind) {
		if self.remaining(available, kind) == 0 {
			return;
		}
		if let Some(slot) = self.slots.iter_mut().find(|slot| slot.is_none()) {
			*slot = Some(kind);
		}
	}

	pub fn remove(&mut self, slot: usize) {
		if let Some(slot) = self.slots.get_mut(slot) {
			*slot = None;
		}
	}

	pub fn clear(&mut self) {
		self.slots.fill(None);
	}

	/// Replaces the deck with `cards`, such as one from the history. Cards that
	/// aren't available any more, or are over their limit, are left out.
	pub fn load(&mut self, available: &[(Card, usize)], cards: &[Card]) {
		let size = self.slots.len();
		self.slots = cards.iter().map(|card| Some(card.kind)).collect();
		self.slots.resize(size, None);
		self.fit(available);
	}

	/// Moves a dragged card. A card from the inventory replaces whatever is in the
	/// slot it's dropped on; a card moved between slots swaps with the card there.
	pub fn drop(&mut self, available: &[(Card, usize)], source: DragSource, target: DropTarget) {
		match (source, target) {
			(DragSource::Inventory(kind), DropTarget::Slot(slot)) => {
				let Some(&current) = self.slots.get(slot) else { return };
				// Replacing a copy of the same card doesn't need a spare copy
				if current != Some(kind) && self.remaining(available, kind) > 0 {
					self.slots[slot] = Some(kind);
				}
			}
			(DragSource::Slot(from), DropTarget::Slot(to)) => {
				if from < self.slots.len() && to < self.slots.len() {
					self.slots.swap(from, to);
				}
			}
			(DragSource::Slot(slot), DropTarget::Inventory) => self.remove(slot),
			(DragSource::Inventory(_), DropTarget::Inventory) => {}
		}
	}

	/// Takes out cards that are no longer available, or that are over their limit,
	/// such as after the facility level or the inventory changes. Later copies go first.
	pub fn fit(&mut self, available: &[(Card, usize)]) {
		for i in (0..self.slots.len()).rev() {
			if let Some(kind) = self.slots[i]
				&& self.count(kind) > limit(available, kind)
			{
				self.slots[i] = None;
			}
		}
	}

	/// The deck's cards at their available levels, sorted, once every slot is filled.
	pub fn deck(&self, available: &[(Card, usize)]) -> Option<Vec<Card>> {
		let mut cards = self
			.slots
			.iter()
			.map(|slot| {
				let kind = (*slot)?;
				available.iter().find(|(card, _)| card.kind == kind).map(|&(card, _)| card)
			})
			.collect::<Option<Vec<Card>>>()?;
		if cards.is_empty() {
			return None;
		}
		cards.sort();
		Some(cards)
	}

	/// The stats for the deck as it is now: asked for, and maybe not back yet.
	pub fn current_stats(&self, available: &[(Card, usize)]) -> Option<&Stats> {
		let stats = self.stats.as_ref()?;
		let current = Some(stats.recipe) == self.recipe && self.deck(available).as_ref() == Some(&stats.cards);
		current.then_some(stats)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use CardKind::*;

	fn available() -> Vec<(Card, usize)> {
		vec![(Card { kind: Cut, level: 4 }, 2), (Card { kind: Season, level: 1 }, 1)]
	}

	fn builder(slots: &[Option<CardKind>]) -> Builder {
		Builder {
			recipe: Some(0),
			slots: slots.to_vec(),
			stats: None,
		}
	}

	#[test]
	fn recipe_sets_slot_count() {
		let mut b = Builder::default();
		b.set_recipe(Some(0), 3);
		assert_eq!(b.slots, vec![None; 3]);
		b.slots = vec![Some(Cut), None, Some(Season)];
		b.set_recipe(Some(1), 2);
		assert_eq!(b.slots, vec![Some(Cut), None]);
		b.set_recipe(None, 5);
		assert!(b.slots.is_empty());
	}

	#[test]
	fn adding_respects_limits_and_space() {
		let mut b = builder(&[None; 4]);
		for _ in 0..3 {
			b.add(&available(), Cut);
		}
		// Only 2 copies of Cut allowed
		assert_eq!(b.slots, vec![Some(Cut), Some(Cut), None, None]);
		assert_eq!(b.remaining(&available(), Cut), 0);
		// Not available at all
		b.add(&available(), Bake);
		assert_eq!(b.count(Bake), 0);
		b.add(&available(), Season);
		assert_eq!(b.slots, vec![Some(Cut), Some(Cut), Some(Season), None]);
	}

	#[test]
	fn dropping_cards() {
		let mut b = builder(&[Some(Cut), Some(Season), None]);
		// Season's only copy is in use
		b.drop(&available(), DragSource::Inventory(Season), DropTarget::Slot(2));
		assert_eq!(b.slots[2], None);
		// Replacing a card frees its copy
		b.drop(&available(), DragSource::Inventory(Cut), DropTarget::Slot(1));
		assert_eq!(b.slots, vec![Some(Cut), Some(Cut), None]);
		// No third Cut, even over a different card
		b.slots[2] = Some(Season);
		b.drop(&available(), DragSource::Inventory(Cut), DropTarget::Slot(2));
		assert_eq!(b.slots[2], Some(Season));
		// Moving between slots swaps
		b.drop(&available(), DragSource::Slot(2), DropTarget::Slot(0));
		assert_eq!(b.slots, vec![Some(Season), Some(Cut), Some(Cut)]);
		// Dropping on the inventory takes the card out
		b.drop(&available(), DragSource::Slot(1), DropTarget::Inventory);
		assert_eq!(b.slots, vec![Some(Season), None, Some(Cut)]);
		// Out of range slots are ignored
		b.drop(&available(), DragSource::Slot(9), DropTarget::Slot(0));
		b.drop(&available(), DragSource::Inventory(Cut), DropTarget::Slot(9));
		assert_eq!(b.slots, vec![Some(Season), None, Some(Cut)]);
	}

	#[test]
	fn fit_takes_out_unavailable_cards() {
		let mut b = builder(&[Some(Cut), Some(Bake), Some(Cut), Some(Cut)]);
		b.fit(&available());
		assert_eq!(b.slots, vec![Some(Cut), None, Some(Cut), None]);
	}

	#[test]
	fn deck_needs_every_slot() {
		let mut b = builder(&[Some(Season), None, Some(Cut)]);
		assert!(!b.is_full());
		assert_eq!(b.deck(&available()), None);
		b.slots[1] = Some(Cut);
		assert!(b.is_full());
		let cut = Card { kind: Cut, level: 4 };
		assert_eq!(b.deck(&available()), Some(vec![cut, cut, Card { kind: Season, level: 1 }]));
		assert_eq!(Builder::default().deck(&available()), None);
	}

	#[test]
	fn loading_a_deck_fits_it() {
		let cut = Card { kind: Cut, level: 9 };
		let mut b = builder(&[None; 3]);
		// A third Cut is over the limit, and a fourth card is past the slots
		b.load(&available(), &[cut, cut, cut, Card { kind: Season, level: 1 }]);
		assert_eq!(b.slots, vec![Some(Cut), Some(Cut), None]);
		b.load(&available(), &[cut]);
		assert_eq!(b.slots, vec![Some(Cut), None, None]);
	}

	#[test]
	fn percentages_add_up_to_100() {
		// Thirds: 33.3% each, and the leftover 0.1% goes to the first
		assert_eq!(tenths_of_percent(&[1, 1, 1]), vec![334, 333, 333]);
		assert_eq!(tenths_of_percent(&[0, 3, 1]), vec![0, 750, 250]);
		// 2/3 has the larger remainder, so it rounds up
		assert_eq!(tenths_of_percent(&[1, 2]), vec![333, 667]);
		assert_eq!(tenths_of_percent(&[0, 0]), vec![0, 0]);
		for counts in [vec![7, 13, 1, 40319], vec![1; 7], vec![5, 0, 9, 2, 11]] {
			assert_eq!(tenths_of_percent(&counts).iter().sum::<u64>(), 1000, "{counts:?}");
		}
	}

	#[test]
	fn history_is_newest_first_without_repeats() {
		let stats = |level: u8, average| DeckStats {
			cards: vec![Card { kind: Cut, level }],
			orders: 1,
			average,
			minimum: Default::default(),
			maximum: Default::default(),
			star_orders: vec![],
		};
		let mut history = Vec::new();
		record(&mut history, stats(1, 10));
		record(&mut history, stats(2, 20));
		record(&mut history, stats(1, 11));
		assert_eq!(history.iter().map(|s| s.average).collect::<Vec<_>>(), vec![11, 20]);
		for level in 0..60 {
			record(&mut history, stats(level, 0));
		}
		assert_eq!(history.len(), HISTORY_LIMIT);
		assert_eq!(history[0].cards[0].level, 59);
	}

	#[test]
	fn stats_only_match_the_same_deck() {
		let mut b = builder(&[Some(Cut), Some(Season)]);
		let cards = b.deck(&available()).unwrap();
		b.stats = Some(Stats { recipe: 0, cards, result: None });
		assert!(b.current_stats(&available()).is_some());
		// A higher card level makes it a different deck
		let upgraded = vec![(Card { kind: Cut, level: 5 }, 2), (Card { kind: Season, level: 1 }, 1)];
		assert!(b.current_stats(&upgraded).is_none());
		b.slots.swap(0, 1);
		assert!(b.current_stats(&available()).is_some(), "slot order doesn't matter");
		b.recipe = Some(1);
		assert!(b.current_stats(&available()).is_none());
	}
}
