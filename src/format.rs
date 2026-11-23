//! Number formatting shared by the command line and the web page.

use crate::builder::tenths_of_percent;
use crate::card::Card;
use crate::deck::DeckStats;

/// `n` with commas between groups of three digits, e.g. 50400 → "50,400".
pub fn group_digits(n: u64) -> String {
	let digits = n.to_string();
	let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
	for (i, digit) in digits.chars().enumerate() {
		if i > 0 && (digits.len() - i).is_multiple_of(3) {
			grouped.push(',');
		}
		grouped.push(digit);
	}
	grouped
}

/// The share of crafts that earned exactly 0, 1, 2… stars, as a percentage and
/// as text to one decimal place. The texts add up to 100%, and a share too small to
/// round to 0.1% shows as "<0.1%" rather than "0.0%".
pub fn star_shares(stats: &DeckStats) -> Vec<(f64, String)> {
	let counts = stats.star_counts();
	let total = stats.crafts().max(1) as f64;
	counts
		.iter()
		.zip(tenths_of_percent(&counts))
		.map(|(&count, tenths)| {
			let shown = if count > 0 && tenths == 0 {
				"<0.1%".to_string()
			} else {
				format!("{}.{}%", tenths / 10, tenths % 10)
			};
			(count as f64 / total * 100.0, shown)
		})
		.collect()
}

/// How a deck was sampled, e.g. "3,360 play orders × 15 rolls = 50,400 crafts".
pub fn sampling(orders: u64, rolls: u64) -> String {
	let plural = |n: u64, one: &str, many: &str| format!("{} {}", group_digits(n), if n == 1 { one } else { many });
	format!(
		"{} × {} = {}",
		plural(orders, "play order", "play orders"),
		plural(rolls, "roll", "rolls"),
		plural(orders * rolls, "craft", "crafts"),
	)
}

/// Sorted cards with repeats grouped, e.g. "3× Heat Control (lvl 7), Cut (lvl 8)".
pub fn grouped_cards(cards: &[Card]) -> String {
	cards
		.chunk_by(|a, b| a == b)
		.map(|copies| match copies.len() {
			1 => copies[0].to_string(),
			count => format!("{count}× {}", copies[0]),
		})
		.collect::<Vec<_>>()
		.join(", ")
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::card::CardKind;

	#[test]
	fn digits() {
		assert_eq!(group_digits(0), "0");
		assert_eq!(group_digits(999), "999");
		assert_eq!(group_digits(50_400), "50,400");
		assert_eq!(group_digits(1_000_000), "1,000,000");
	}

	#[test]
	fn sampling_text() {
		assert_eq!(sampling(3360, 15), "3,360 play orders × 15 rolls = 50,400 crafts");
		assert_eq!(sampling(1, 1), "1 play order × 1 roll = 1 craft");
	}

	#[test]
	fn cards_grouped() {
		let heat = Card { kind: CardKind::HeatControl, level: 7 };
		let cut = Card { kind: CardKind::Cut, level: 8 };
		assert_eq!(grouped_cards(&[heat, heat, heat, cut]), "3× Heat Control (lvl 7), Cut (lvl 8)");
	}
}
