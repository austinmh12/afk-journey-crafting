//! Where the web page looks for each image, under the `static` folder.
//!
//! Files are named after the item's name in lowercase, with every run of
//! characters other than letters and digits replaced by `-`:
//!
//! - `static/facilities/kitchen.png`
//! - `static/cards/heat-control.png`, `static/cards/multi-forge.png`
//! - `static/recipes/round-bread.png`, `static/recipes/shroom-stir-fry.png`
//!
//! Paths are relative, so they work when the site is served from a sub-path
//! such as GitHub Pages' `/<repository>/`.

use crate::card::CardKind;
use crate::recipe::{Location, Recipe};

const EXTENSION: &str = "png";

pub fn facility(location: Location) -> String {
	path("facilities", location.name())
}

pub fn card(kind: CardKind) -> String {
	path("cards", kind.name())
}

pub fn recipe(recipe: &Recipe) -> String {
	path("recipes", &recipe.name)
}

fn path(folder: &str, name: &str) -> String {
	format!("static/{folder}/{}.{EXTENSION}", slug(name))
}

/// `name` in lowercase with each run of other characters than letters and digits
/// turned into one `-`, e.g. "Shroom Stir-Fry" → "shroom-stir-fry".
pub fn slug(name: &str) -> String {
	let mut slug = String::with_capacity(name.len());
	for c in name.chars() {
		if c.is_alphanumeric() {
			slug.extend(c.to_lowercase());
		} else if !slug.is_empty() && !slug.ends_with('-') {
			slug.push('-');
		}
	}
	slug.trim_end_matches('-').to_string()
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn slugs() {
		assert_eq!(slug("Round Bread"), "round-bread");
		assert_eq!(slug("Shroom Stir-Fry"), "shroom-stir-fry");
		assert_eq!(slug("Multi-Forge"), "multi-forge");
		assert_eq!(slug("  Odd -- Sweet! "), "odd-sweet");
	}

	#[test]
	fn paths() {
		assert_eq!(facility(Location::Alchemy), "static/facilities/alchemy.png");
		assert_eq!(card(CardKind::HeatControl), "static/cards/heat-control.png");
	}
}
