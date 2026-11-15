mod card;
mod deck;
mod recipe;
mod score;
mod tables;

use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use serde::Deserialize;
use serde::de::DeserializeOwned;

use card::{Card, CardKind};
use deck::{Deck, ScoreResults, generate_decks};
use recipe::{Location, Recipe};
use tables::CardTables;

/// Finds the best deck for each AFK Journey homestead crafting recipe.
#[derive(Debug, Parser)]
#[command(version, about)]
struct Args {
	/// Only run this recipe (case-insensitive). Takes priority over --location.
	#[arg(short, long)]
	recipe: Option<String>,

	/// Only run recipes at this location.
	#[arg(short, long, ignore_case = true)]
	location: Option<Location>,

	/// Inventory file to use [default: <DATA_DIR>/inventory.ron]
	#[arg(short, long)]
	inventory: Option<PathBuf>,

	/// Directory holding cards.ron, recipes.ron and the default inventory.ron.
	#[arg(short, long, default_value = "data")]
	data_dir: PathBuf,
}

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
fn main() -> ExitCode {
	match run(Args::parse()) {
		Ok(()) => ExitCode::SUCCESS,
		Err(e) => {
			eprintln!("error: {e}");
			ExitCode::FAILURE
		}
	}
}

fn run(args: Args) -> Result<(), Box<dyn Error>> {
	let tables: CardTables = load(&args.data_dir.join("cards.ron"))?;
	let inventory_path = args.inventory.unwrap_or_else(|| args.data_dir.join("inventory.ron"));
	let inventory: Inventory = load(&inventory_path)?;
	let recipes_path = args.data_dir.join("recipes.ron");
	let recipes: Vec<Recipe> = load(&recipes_path)?;

	let selected: Vec<&Recipe> = if let Some(name) = &args.recipe {
		if args.location.is_some() {
			eprintln!("Both --recipe and --location given: running only the recipe");
		}
		match recipes.iter().find(|r| r.name.eq_ignore_ascii_case(name)) {
			Some(recipe) => vec![recipe],
			None => return Err(format!("no recipe named \"{name}\" in {}", recipes_path.display()).into()),
		}
	} else if let Some(location) = args.location {
		recipes.iter().filter(|r| r.location == location).collect()
	} else {
		recipes.iter().collect()
	};

	let mut results: Vec<(&Recipe, Deck, ScoreResults)> = Vec::new();

	for recipe in selected {
		let decks = generate_decks(&available_cards(&inventory, recipe.location), recipe.cards);
		println!("{} has {} unique decks", recipe.name, decks.len());

		let progress = ProgressBar::new(decks.len() as u64)
			.with_style(ProgressStyle::with_template("{msg}: {wide_bar} {pos}/{len} [{elapsed_precise}<{eta_precise}]")?)
			.with_message(recipe.name.clone());

		// Score decks in parallel, each worker thread with its own RNG
		let best = decks
			.into_par_iter()
			.enumerate()
			.map_init(rand::rng, |rng, (index, deck)| {
				let scored = deck.score(recipe, &tables, rng);
				progress.inc(1);
				(index, deck, scored)
			})
			// Highest average wins; on ties keep the earliest deck, as a sequential run would
			.reduce_with(|a, b| {
				let (a_avg, b_avg) = (a.2.average(), b.2.average());
				if b_avg > a_avg || (b_avg == a_avg && b.0 < a.0) { b } else { a }
			});
		progress.finish();

		if let Some((_, deck, scored)) = best {
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
