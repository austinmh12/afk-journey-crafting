mod card;
mod deck;
mod recipe;
mod score;
mod tables;

use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use indicatif::{ProgressBar, ProgressStyle};
use serde::Deserialize;
use serde::de::DeserializeOwned;

use card::{Card, CardKind};
use deck::{Deck, ScoreResults, generate_decks};
use recipe::{Location, Recipe};
use tables::CardTables;

/// How many copies of a card may be used, and its current level.
#[derive(Debug, Clone, Copy, Deserialize)]
struct Owned {
	limit: usize,
	level: u8,
}

type Inventory = BTreeMap<Location, BTreeMap<CardKind, Owned>>;

fn load<T: DeserializeOwned>(path: &Path) -> Result<T, Box<dyn Error>> {
	let text = fs::read_to_string(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
	let value = ron::from_str(&text).map_err(|e| format!("parsing {}: {e}", path.display()))?;
	Ok(value)
}

fn available_cards(inventory: &Inventory, location: Location) -> Vec<(Card, usize)> {
	inventory
		.get(&location)
		.into_iter()
		.flatten()
		.map(|(&kind, owned)| (Card { kind, level: owned.level }, owned.limit))
		.collect()
}

// TODO: Add modes for selecting highest minimum for highest chance at stars, etc.
fn main() -> Result<(), Box<dyn Error>> {
	// Data directory defaults to ./data, or pass a path as the first argument
	let data_dir = std::env::args().nth(1).map_or_else(|| PathBuf::from("data"), PathBuf::from);
	let tables: CardTables = load(&data_dir.join("cards.ron"))?;
	let inventory: Inventory = load(&data_dir.join("inventory.ron"))?;
	let recipes: Vec<Recipe> = load(&data_dir.join("recipes.ron"))?;

	let mut rng = rand::rng();
	let mut results: Vec<(&Recipe, Deck, ScoreResults)> = Vec::new();

	for recipe in &recipes {
		let decks = generate_decks(&available_cards(&inventory, recipe.location), recipe.cards);
		println!("{} has {} unique decks", recipe.name, decks.len());

		let progress = ProgressBar::new(decks.len() as u64)
			.with_style(ProgressStyle::with_template("{msg}: {wide_bar} {pos}/{len} [{elapsed_precise}<{eta_precise}]")?)
			.with_message(recipe.name.clone());

		// Only fully score decks whose sample average is at least the best seen so far
		let mut best: Option<(Deck, ScoreResults)> = None;
		for deck in decks {
			progress.inc(1);
			let scored = deck.score(&recipe.name, &tables, &mut rng);
			// Keep the first deck on ties
			if best.as_ref().is_none_or(|(_, b)| scored.average() > b.average()) {
				best = Some((deck, scored));
			}
		}
		progress.finish();

		if let Some((deck, scored)) = best {
			results.push((recipe, deck, scored));
		}
	}

	for (recipe, deck, result) in results {
		let stars = |value: i64| "*".repeat(recipe.stars(value));
		let average = result.average();
		println!("{}", recipe.name);
		println!("    Min: {} {}", result.minimum, stars(result.minimum.total()));
		println!("    Avg: {} {}", average, stars(average));
		println!("    Max: {} {}", result.maximum, stars(result.maximum.total()));
		println!("    Cards for Average:");
		let cards: Vec<String> = deck.cards.iter().map(Card::to_string).collect();
		println!("      {}", cards.join(", "));
	}
	Ok(())
}
