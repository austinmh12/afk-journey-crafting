# AFK Journey Homestead Crafting Calculator

Finds the best deck of cards to craft each AFK Journey homestead recipe with, based on the cards you own and their levels.

For each recipe, the calculator:

1. Builds every distinct deck of the recipe's card count from your inventory for that location (Kitchen, Forge or Alchemy), using each card no more times than your limit allows.
2. Scores every distinct play order of each deck, applying card effects (Charge, Multi-Forge, Heat Up, Reversal, Banquet and so on) and the recipe's power, if it has one.
3. Reports the deck with the highest **average** score across its play orders, along with that deck's minimum and maximum, and the stars each one earns. This is because I prioritize regularly getting stars rather than going for the highest value possible.

Decks are scored in parallel across all CPU cores.

There are two ways to use it: a [web page](#web-front-end) where you edit your cards and pick recipes, or the [command line](#running).

A big thanks to [@MatsuTaku](https://github.com/MatsuTaku) as they had a lot of the data that made it possible to score earlier levels of cards. They made a tool here: [AFK Journey Techcard Optimizer](https://github.com/MatsuTaku/afkjourney-techcard-optimizer). Though slightly outdated, the information is still good for early to mid-level homesteads.

## Requirements

- Rust 1.85 or newer (the crate uses the 2024 edition). Install it from <https://rustup.rs>.

## Web front end

The web front end is built with [Yew](https://yew.rs) and [Trunk](https://trunkrs.dev). It runs entirely in your browser:

- A tab for each location (Kitchen, Forge, Alchemy) at the top of the page.
- The location's cards, each with a **Level** and **Limit** box. A limit of 0 leaves the card out. Edits are saved in your browser, and **Reset all locations to defaults** goes back to `data/inventory.ron`.
- A **Recipe** selector. **All** (the default) runs every recipe at the location; or pick a single recipe. **Calculate** runs it.
- A results table with one row per recipe: the recipe, with its average score and stars underneath, and the cards in its best deck.

Decks are scored in [web workers](https://developer.mozilla.org/en-US/docs/Web/API/Web_Workers_API), one per CPU core, so the page stays responsive during long runs. Expect it to be slower than the command line, as WebAssembly is slower than native code.

### Setup

```sh
rustup target add wasm32-unknown-unknown
cargo install --locked trunk
```

### Running the web front end

From the repository root:

```sh
# Builds, serves on http://127.0.0.1:8080 and rebuilds on changes
trunk serve --release

# Or build static files into dist/ to host anywhere
trunk build --release
```

The `data/*.ron` files are built into the web app, so rebuild after editing them.

## Running

Build and run with Cargo from the repository root. Always use `--release` when timing or running large recipes: the debug build is several times slower.

```sh
# Every recipe in data/recipes.ron
cargo run --release

# Arguments for the calculator go after `--`
cargo run --release -- --recipe "Fireward Ring"
```

Or build once and run the binary directly:

```sh
cargo build --release
./target/release/afkjourney-crafting --location forge
```

Run it from the repository root, or pass `--data-dir`, so it can find the `data` folder.

## Command line options

| Option | Description |
|---|---|
| `-r`, `--recipe <RECIPE>` | Run only this recipe. The name must match one in `recipes.ron`, ignoring case. Exits with an error if there's no match. Takes priority over `--location`. |
| `-l`, `--location <LOCATION>` | Run only recipes at this location: `kitchen`, `forge` or `alchemy` (any case). Ignored when `--recipe` is also given. |
| `-i`, `--inventory <FILE>` | Use this inventory file instead of the default `<DATA_DIR>/inventory.ron`. Useful for trying out card upgrades or comparing accounts. |
| `-d`, `--data-dir <DIR>` | Folder holding `cards.ron`, `recipes.ron` and the default `inventory.ron`. Defaults to `data`. |
| `-h`, `--help` | Print help. |
| `-V`, `--version` | Print the version. |

With no options, every recipe in `recipes.ron` is run.

### Examples

```sh
# One recipe
cargo run --release -- -r "copper carver"

# All Alchemy recipes
cargo run --release -- -l alchemy

# What if I upgraded some cards?
cargo run --release -- -l kitchen -i data/inventory-upgraded.ron
```

## Reading the output

```text
Round Bread has 86 unique decks
Round Bread: ████████████████████████████████████ 86/86 [00:00:00<00:00:00]
Round Bread
    Min: 76x61 (4636) **
    Avg: 4636 **
    Max: 76x61 (4636) **
    Cards for Average:
      Heat Control (lvl 7), Heat Control (lvl 7), Heat Control (lvl 7)
```

- **Min / Max** show the two colour values as `left x right`, then the final score (their product) in brackets.
- **Avg** is the average final score over every play order of the chosen deck.
- The `*`s are the stars that score earns, from the recipe's thresholds.
- **Cards for Average** is the winning deck. The order shown is not a recommended play order.

Summaries for all recipes are printed together after the last progress bar finishes.

## Data files

All game data lives in [RON](https://github.com/ron-rs/ron) files in `data/`, so values can be changed without recompiling. Lines starting with `//` are comments.

### `inventory.ron`: your cards

For each location, the cards you own: how many copies may go in one deck (`limit`) and the card's current `level`.

```ron
{
	Kitchen: {
		HeatControl: (limit: 4, level: 7),
		Cut: (limit: 4, level: 8),
		// ...
	},
	Forge: { /* ... */ },
	Alchemy: { /* ... */ },
}
```

A card left out of a location is never used there. Card names are the `CardKind` names from [src/card.rs](src/card.rs), such as `HeatControl`, `MultiForge` and `ForgeExpert`.

### `recipes.ron`: what to craft

```ron
[
	// Power: +15 to random color at the start of production
	(name: "Fireward Ring", location: Alchemy, cards: 9, stars: [6200, 8000, 10200, 12120]),
]
```

- `cards` is the number of cards played in one craft.
- `stars` are the score thresholds for each star, lowest first.
- Comment a recipe out with `//` to skip it in full runs.

Recipe powers are matched by recipe name in code (see the `powers` module in [src/recipe.rs](src/recipe.rs)). The `// Power:` comments are notes only. Adding a comment does not add a power.

### `cards.ron`: card values by level

Each card's values per level, with a `default` used for any level that isn't listed:

```ron
heat_up: (
	default: 0,
	levels: { 1: 3, 2: 6, 3: 10, 4: 12, 5: 14 },
),
```

Edit this file when a card's in-game numbers are confirmed or change.

## Project layout

| Path | Contents |
|---|---|
| [src/main.rs](src/main.rs) | Entry point: the command line calculator natively, the web app in the browser. |
| [src/cli.rs](src/cli.rs) | Command line options, loading data, running recipes and printing results. |
| [src/lib.rs](src/lib.rs) | The scoring library shared by the command line and the web app. |
| [src/inventory.rs](src/inventory.rs) | The owned cards in `inventory.ron`. |
| [src/web/app.rs](src/web/app.rs) | The Yew page: location tabs, card inputs, recipe selector and results table. |
| [src/web/worker.rs](src/web/worker.rs), [src/bin/worker.rs](src/bin/worker.rs) | The web worker that scores decks. |
| [index.html](index.html), [style.css](style.css) | The page shell Trunk builds from, and its styles. |
| [src/deck.rs](src/deck.rs) | Generating decks and play orders, and scoring them. |
| [src/card.rs](src/card.rs) | Every card type and its effect. |
| [src/recipe.rs](src/recipe.rs) | Recipes, locations and recipe powers. |
| [src/score.rs](src/score.rs) | The two-colour score and the modifier stack that cards build up during a craft. |
| [src/tables.rs](src/tables.rs) | Types for the per-level values in `cards.ron`. |

Run the tests with `cargo test`.

## Limitations

- Some card values in `cards.ron` are guesses or placeholders, marked `TODO` or `A guess`. This is because the AFK Journey wiki does not contain this information, nor can you view the previous levels in the game, only the current and next levels. A lot of the values I found prior to my levels are from [@MatsuTaku's](https://github.com/MatsuTaku) [AFK Journey Techcard Optimizer](https://github.com/MatsuTaku/afkjourney-techcard-optimizer), some of which were outdated to do patches.
- Facility level limits are not modelled. Set `limit` and `level` in `inventory.ron` to what your facility currently allows. This is a plan to do, [@MatsuTaku's](https://github.com/MatsuTaku) repo has some of this info. Once parsed into a format for this script, I'll update the command line args to allow for location levels
- Cards with random effects (Heat Control, Cut, random-colour hits) are scored once per distinct play order, so a deck with few distinct orders, such as three copies of one card, gets few random samples.

## Roadmap Items

- Add different scoring modes such as the highest scoring deck for a recipe or the lowest scoring, etc.
- Add visualization of the decks being scored to the web front end