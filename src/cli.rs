//! The command line calculator.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use serde::de::DeserializeOwned;

use afkjourney_crafting::deck::{DeckStats, beats, generate_decks};
use afkjourney_crafting::facility::{Facilities, MAX_LEVEL};
use afkjourney_crafting::format::{grouped_cards, sampling, star_shares};
use afkjourney_crafting::inventory::{available_cards, maxed_inventory};
use afkjourney_crafting::recipe::{Location, Recipe};
use afkjourney_crafting::tables::CardTables;

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

	/// Kitchen facility level. Every Kitchen card is set to the most this level allows.
	#[arg(short, long, value_name = "LEVEL", default_value_t = 1, value_parser = facility_level)]
	kitchen: u8,

	/// Forge facility level. Every Forge card is set to the most this level allows.
	#[arg(short, long, value_name = "LEVEL", default_value_t = 1, value_parser = facility_level)]
	forge: u8,

	/// Alchemy facility level. Every Alchemy card is set to the most this level allows.
	#[arg(short, long, value_name = "LEVEL", default_value_t = 1, value_parser = facility_level)]
	alchemy: u8,

	/// Directory holding cards.ron, recipes.ron and facilities.ron.
	#[arg(short, long, default_value = "data")]
	data_dir: PathBuf,

	/// About how many crafts to score each best deck with, for its average, lowest
	/// and highest scores and chance of each star. Every play order is scored the
	/// same number of times, and at least once.
	#[arg(short, long, default_value_t = 50_000, value_parser = clap::value_parser!(u64).range(1..))]
	samples: u64,
}

/// Parses a facility level, from 1 to the highest in the game.
fn facility_level(text: &str) -> Result<u8, String> {
	match text.parse::<u8>() {
		Ok(level) if (1..=MAX_LEVEL).contains(&level) => Ok(level),
		_ => Err(format!("must be a facility level from 1 to {MAX_LEVEL}")),
	}
}

impl Args {
	/// The facility level for `location`.
	fn level(&self, location: Location) -> u8 {
		match location {
			Location::Kitchen => self.kitchen,
			Location::Forge => self.forge,
			Location::Alchemy => self.alchemy,
		}
	}
}

fn load<T: DeserializeOwned>(path: &Path) -> Result<T, Box<dyn Error>> {
	let text = fs::read_to_string(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
	let value = ron::from_str(&text).map_err(|e| format!("parsing {}: {e}", path.display()))?;
	Ok(value)
}

pub fn main() -> ExitCode {
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
	let recipes_path = args.data_dir.join("recipes.ron");
	let recipes: Vec<Recipe> = load(&recipes_path)?;
	let facilities: Facilities = load(&args.data_dir.join("facilities.ron"))?;
	// Every card maxed for its facility's level
	let inventory = maxed_inventory(&facilities, |location| args.level(location));

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

	// Leave out recipes the facility's level hasn't unlocked, unless asked for by name
	let unlocked = |recipe: &Recipe| {
		inventory
			.get(&recipe.location)
			.is_none_or(|facility| facility.recipe_unlocked(&facilities, recipe))
	};
	let (selected, locked): (Vec<&Recipe>, Vec<&Recipe>) =
		selected.into_iter().partition(|recipe| args.recipe.is_some() || unlocked(recipe));
	for recipe in &selected {
		if !unlocked(recipe) {
			eprintln!("{} isn't unlocked at your {} level yet, running it anyway", recipe.name, recipe.location.name());
		}
	}
	if !locked.is_empty() {
		let names: Vec<&str> = locked.iter().map(|recipe| recipe.name.as_str()).collect();
		eprintln!("Skipping recipes not unlocked at your facility level: {}", names.join(", "));
	}

	let mut locations: Vec<Location> = selected.iter().map(|recipe| recipe.location).collect();
	locations.sort();
	locations.dedup();
	for location in locations {
		eprintln!("{} level {}", location.name(), args.level(location));
	}

	let mut results = Vec::new();

	for recipe in selected {
		let decks = generate_decks(&available_cards(&inventory, &facilities, recipe.location), recipe.cards);
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
			.reduce_with(|a, b| if beats(b.2.average(), b.0, a.2.average(), a.0) { b } else { a });
		progress.finish();

		match best {
			Some((_, deck, _)) => results.push((recipe, Some(deck))),
			None => results.push((recipe, None)),
		}
	}

	// Score each best deck again with the samples setting, like the web page,
	// for a steadier average and its chance of each star
	let progress = ProgressBar::new(results.iter().filter(|(_, deck)| deck.is_some()).count() as u64)
		.with_style(ProgressStyle::with_template("{msg}: {wide_bar} {pos}/{len} [{elapsed_precise}<{eta_precise}]")?)
		.with_message("Sampling best decks");
	let stats: Vec<(&Recipe, Option<DeckStats>)> = results
		.into_par_iter()
		.map_init(rand::rng, |rng, (recipe, deck)| {
			let stats = deck.map(|deck| {
				let stats = deck.stats(recipe, &tables, args.samples, rng);
				progress.inc(1);
				stats
			});
			(recipe, stats)
		})
		.collect();
	progress.finish();

	for (recipe, stats) in stats {
		println!();
		print_stats(recipe, stats.as_ref());
	}
	Ok(())
}

/// Prints a recipe's best deck and its stats, as the web page's Best decks table shows them.
fn print_stats(recipe: &Recipe, stats: Option<&DeckStats>) {
	println!("{}", recipe.name);
	let Some(stats) = stats else {
		println!("    Not enough cards: needs {}", recipe.cards);
		return;
	};
	let stars = |score: i64| {
		let stars = recipe.stars(score);
		if stars == 0 { String::new() } else { format!(" {}", "*".repeat(stars)) }
	};
	let (low, high) = (&stats.minimum, &stats.maximum);
	println!("    Avg:    {}{}", stats.average, stars(stats.average));
	println!("    Low:    {}x{} ({}){}", low.left, low.right, low.total(), stars(low.total()));
	println!("    High:   {}x{} ({}){}", high.left, high.right, high.total(), stars(high.total()));
	let shares: Vec<String> = star_shares(stats)
		.into_iter()
		.enumerate()
		.map(|(count, (_, shown))| format!("{count}* {shown}"))
		.collect();
	println!("    Stars:  {}", shares.join(" | "));
	println!("    Scored: {}", sampling(stats.orders, stats.rolls));
	println!("    Cards:  {}", grouped_cards(&stats.cards));
}
