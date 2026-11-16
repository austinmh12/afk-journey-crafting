//! The web front end, built with [Yew](https://yew.rs) and `trunk`.

mod app;
pub mod worker;

pub use app::App;

/// Game data, built into the web app from the `data` folder at compile time.
pub mod data {
	use crate::inventory::Inventory;
	use crate::recipe::Recipe;
	use crate::tables::CardTables;

	pub fn tables() -> CardTables {
		ron::from_str(include_str!("../../data/cards.ron")).expect("data/cards.ron should parse")
	}

	pub fn recipes() -> Vec<Recipe> {
		ron::from_str(include_str!("../../data/recipes.ron")).expect("data/recipes.ron should parse")
	}

	/// The starting inventory, used until the player edits theirs.
	pub fn default_inventory() -> Inventory {
		ron::from_str(include_str!("../../data/inventory.ron")).expect("data/inventory.ron should parse")
	}
}
