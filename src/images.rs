//! Where the web page looks for each image, under the `static` folder.
//!
//! Files are named after the item's name in PascalCase: each word starts with
//! a capital letter and every character other than a letter or digit is
//! dropped. Every image is a PNG:
//!
//! - `static/facilities/Kitchen.png`
//! - `static/cards/HeatControl.png`, `static/cards/MultiForge.png`
//! - `static/recipes/RoundBread.png`, `static/recipes/ShroomStirFry.png`
//!
//! Paths are relative, so they work when the site is served from a sub-path
//! such as GitHub Pages' `/<repository>/`.

use crate::card::CardKind;
use crate::recipe::{Location, Recipe};

pub fn facility(location: Location) -> String {
	path("facilities", location.name(), "png")
}

pub fn card(kind: CardKind) -> String {
	path("cards", kind.name(), "png")
}

pub fn recipe(recipe: &Recipe) -> String {
	path("recipes", &recipe.name, "png")
}

fn path(folder: &str, name: &str, extension: &str) -> String {
	format!("static/{folder}/{}.{extension}", file_stem(name))
}

/// `name` in PascalCase with every character other than a letter or digit
/// dropped, e.g. "Shroom Stir-Fry" → "ShroomStirFry".
pub fn file_stem(name: &str) -> String {
	let mut stem = String::with_capacity(name.len());
	let mut word_start = true;
	for c in name.chars() {
		if c.is_alphanumeric() {
			if word_start {
				stem.extend(c.to_uppercase());
			} else {
				stem.push(c);
			}
			word_start = false;
		} else {
			word_start = true;
		}
	}
	stem
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn file_stems() {
		assert_eq!(file_stem("Round Bread"), "RoundBread");
		assert_eq!(file_stem("Shroom Stir-Fry"), "ShroomStirFry");
		assert_eq!(file_stem("Multi-Forge"), "MultiForge");
		assert_eq!(file_stem("Anti-Itch Roller"), "AntiItchRoller");
		assert_eq!(file_stem("  odd -- sweet! "), "OddSweet");
	}

	#[test]
	fn paths() {
		assert_eq!(facility(Location::Alchemy), "static/facilities/Alchemy.png");
		assert_eq!(card(CardKind::HeatControl), "static/cards/HeatControl.png");
		assert_eq!(card(CardKind::MultiForge), "static/cards/MultiForge.png");
	}

	/// Lists every card, facility and recipe image the page expects but that
	/// isn't in `static/`. Run with `cargo test missing_images -- --ignored --nocapture`.
	#[test]
	#[ignore]
	fn missing_images() {
		let recipes: Vec<Recipe> =
			ron::from_str(include_str!("../data/recipes.ron")).expect("recipes.ron parses");
		let expected = [Location::Kitchen, Location::Forge, Location::Alchemy]
			.into_iter()
			.map(facility)
			.chain(CardKind::ALL.into_iter().map(card))
			.chain(recipes.iter().map(recipe));
		let missing: Vec<String> = expected
			.filter(|path| !std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path).exists())
			.collect();
		for path in &missing {
			println!("missing: {path}");
		}
		assert!(missing.is_empty(), "{} images missing", missing.len());
	}
}
