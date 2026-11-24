# AFK Journey Homestead Crafting Calculator

Finds the best deck of cards to craft each AFK Journey homestead recipe with, based on the cards you own and their levels.

For each recipe, the calculator:

1. Builds every distinct deck of the recipe's card count from your cards at that location (Kitchen, Forge or Alchemy), using each card no more times than its limit allows.
2. Scores every distinct play order of each deck, applying card effects (Charge, Multi-Forge, Heat Up, Reversal, Banquet and so on) and the recipe's power, if it has one.
3. Picks the deck with the highest **average** score across its play orders. This is because I prioritize regularly getting stars rather than going for the highest value possible.
4. Scores that deck again with about `--samples` crafts (50,000 by default), and reports its average, lowest and highest scores, and its chance of earning each number of stars.

Decks are scored in parallel across all CPU cores.

There are two ways to use it: a [web page](#web-front-end) where you edit your cards and pick recipes, or the [command line](#running).

A big thanks to [@MatsuTaku](https://github.com/MatsuTaku) as they had a lot of the data that made it possible to score earlier levels of cards. They made a tool here: [AFK Journey Techcard Optimizer](https://github.com/MatsuTaku/afkjourney-techcard-optimizer). Though slightly outdated, the information is still good for early to mid-level homesteads.

Also a big thanks to the [Yaphalla Community](https://www.google.com/url?q=http://discord.gg/yaphalla&sa=D&source=editors&ust=1795464634893809&usg=AOvVaw1OU8FqSi7CmPesYzthUfyO) for their work on compiling information in the [AFK Journey 1.6.1 Homestead Spreadsheet](https://docs.google.com/spreadsheets/d/e/2PACX-1vTkwoy0_IKEWZZWoCebVXn3zNEVOGbAvTdtn4d1tznfqT5n9SdnKM0oIIo6fCDFV2SharTIEnOx91v3/pubhtml#gid=1017644258).

## Requirements

- Rust 1.85 or newer (the crate uses the 2024 edition). Install it from <https://rustup.rs>.

## Web front end

**Use it online: <https://austinmh12.github.io/afk-journey-crafting/>**

The web front end is built with [Yew](https://yew.rs) and [Trunk](https://trunkrs.dev). It runs entirely in your browser:

- A sidebar on the left with the four pages: **Deck builder**, which the app opens on, **Best decks**, **Cards** and **Facilities**. The current page is highlighted, and its name heads the page. On narrow screens the pages become a bar across the top.
- A tab for each location (Kitchen, Forge, Alchemy) at the top of the page.
- A **Facility level** selector. Cards are capped to what that level allows: capped values are flagged (for example "Level → 4"), cards that aren't unlocked yet are greyed out, and **Use level maximums** sets every card to the highest it can be. Recipes the level hasn't unlocked are left out of the recipe selector and **All**.
- The location's cards, each with a **Level** and **Limit** box. A limit of 0 leaves the card out. Edits are saved in your browser, and **Reset all locations to defaults** goes back to the starting cards: every facility at level 1, with each card at the most that level allows.
- A **Recipe** selector. **All** (the default) runs every recipe at the location; or pick a single recipe. **Calculate** runs it.
- A **Samples** setting, shared with the deck builder (see below).
- A results table with one row per recipe: the recipe, with its average score and stars underneath, and the cards in its best deck. Finding the best deck scores each play order of each deck once; once a recipe's best deck is found, it's scored again with the samples setting, and the row shows that steadier average, a bar with its chance of earning exactly 0, 1, 2… stars, and its lowest and highest scores seen.
- The **Deck builder** page, for scoring a deck of your own:
  - Pick a recipe from the location to see its image, card count, star thresholds and power, if it has one.
  - Drag cards from the location's available cards into the deck's empty slots, or click a card to fill the next slot. Each card shows how many copies are left. Drag cards between slots to swap them, and drag one back to the available cards, or click it, to take it out.
  - Once every slot is filled, **Score deck** scores every distinct play order and shows the average, lowest and highest scores with their stars, the share of play orders that earn exactly 0, 1, 2… stars (adding up to 100%), and the highest- and lowest-scoring play orders.
  - Random effects (Cut rolls, random colours, Heat Control retriggers and so on) make one craft of a play order just one sample of what it can score. To keep that noise about the same for every deck, the **Samples** setting (10,000 to 1,000,000, default 50,000) sets about how many crafts to score: every play order is scored the same number of times, and at least once. A deck with many play orders gets a roll or two of each; a deck of five identical cards has one play order, rolled 50,000 times. The status line shows the plan before scoring, such as "3,360 play orders × 15 rolls = 50,400 crafts". Lowest and highest are the extremes over every craft, so they spread wider with more samples. The setting is saved in your browser.
  - It also compares your deck with the best deck for the recipe: both averages, the difference, and the best deck's cards. Once the best deck is found, it's also scored with your **Samples** setting, so the averages compare like for like, and its star chances are drawn beside yours. The search itself scores each play order once; until the best deck has been sampled (or after changing the setting), **Sample** next to its average does it. If the calculator hasn't already found the best deck for your current cards, scoring a deck starts that search in the background (it can be stopped, and restarted with **Find best deck**).
  - The **Scoring history** column on the right lists every deck scored for the recipe, newest first: its cards, average, lowest and highest scores, chance of the top star, and how far it is from the best deck. It stays in view as you scroll, and moves below the builder on narrow screens. **Load** puts a deck back in the builder. Up to 50 decks per recipe are saved in your browser.

- The **Cards** page, a guide to the location's cards, in the order they unlock. Where two unlock at the same facility level, a card that scores comes before one that only changes how other cards score. Each card has a section with:
  - What it does: how it scores, or how it changes other cards.
  - The facility level it unlocks at.
  - A table with a row for each facility level where the card unlocks, levels up or allows more copies per deck: the card level, the copies, and the card's values at that level (points, charges, multipliers, debuffs and so on). What changed is in bold. With a facility level set, the rows it hasn't reached are faded and the row for your level is highlighted.
- The **Facilities** page, with a row for each level of the location's facility: the recipes it unlocks, and the cards it unlocks (marked **New**), levels up or allows more copies of. With a facility level set, your level's row is highlighted.

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

### Hosting on GitHub Pages

[.github/workflows/pages.yml](.github/workflows/pages.yml) runs the tests, builds the page and publishes it to GitHub Pages on every push to `trunk`. It can also be run by hand from the Actions tab. One-time setup: in the repository's **Settings > Pages**, set **Source** to **GitHub Actions**.

Pages serves the site from `/<repository name>/`, which the workflow passes to Trunk as `--public-url`. To try that locally:

```sh
trunk build --release --public-url /afk-journey-crafting/
```

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
./target/release/afkjourney-crafting --location forge --forge 11
```

Run it from the repository root, or pass `--data-dir`, so it can find the `data` folder.

## Command line options

| Option | Description |
|---|---|
| `-r`, `--recipe <RECIPE>` | Run only this recipe. The name must match one in `recipes.ron`, ignoring case. Exits with an error if there's no match. Takes priority over `--location`. |
| `-l`, `--location <LOCATION>` | Run only recipes at this location: `kitchen`, `forge` or `alchemy` (any case). Ignored when `--recipe` is also given. |
| `-k`, `--kitchen <LEVEL>` | Your Kitchen's facility level, from 1 to 20. Defaults to 1. |
| `-f`, `--forge <LEVEL>` | Your Forge's facility level, from 1 to 20. Defaults to 1. |
| `-a`, `--alchemy <LEVEL>` | Your Alchemy facility level, from 1 to 20. Defaults to 1. |
| `-d`, `--data-dir <DIR>` | Folder holding `cards.ron`, `recipes.ron` and `facilities.ron`. Defaults to `data`. |
| `-s`, `--samples <SAMPLES>` | About how many crafts to score each best deck with, for its stats. Every play order is scored the same number of times, and at least once. Defaults to 50,000, the web page's default. More is steadier but slower. |
| `-h`, `--help` | Print help. |
| `-V`, `--version` | Print the version. |

Each facility level sets every card at that location to the highest level and most copies the level allows, from [`facilities.ron`](#facilitiesron-what-each-facility-level-allows), and leaves out cards it hasn't unlocked. The levels in use are printed before the run.

With no options, every recipe unlocked at facility level 1 is run.

### Examples

```sh
# One recipe, with a level 5 Forge
cargo run --release -- -r "copper carver" -f 5

# All Alchemy recipes unlocked at Alchemy level 11
cargo run --release -- -l alchemy -a 11

# Every unlocked recipe, with your level of each facility
cargo run --release -- -k 12 -f 11 -a 11

# Steadier stats for the best decks
cargo run --release -- -l forge -s 1000000
```

## Reading the output

```text
Fireward Ring has 7859 unique decks
Fireward Ring: ██████████████████████████████████ 7859/7859 [00:00:01<00:00:00]
Sampling best decks: ███████████████████████████████████████ 1/1 [00:00:00<00:00:00]

Fireward Ring
    Avg:    59948 ****
    Low:    279x49 (13671) ****
    High:   478x302 (144356) ****
    Stars:  0* 0.0% | 1* 0.0% | 2* 0.0% | 3* 0.0% | 4* 100.0%
    Scored: 45,360 play orders × 2 rolls = 90,720 crafts
    Cards:  2× Grind (lvl 8), Distill (lvl 1), Crystallize (lvl 1), 2× Overload (lvl 2), 2× Enchant (lvl 4), Reversal (lvl 1)
```

These are the same stats the web page's **Best decks** table shows.

- **Avg** is the best deck's average final score over every craft it was sampled with.
- **Low / High** are the lowest and highest scoring crafts seen, as the two colour values `left x right`, then the final score (their product) in brackets. They spread wider with more samples.
- The `*`s are the stars a score earns, from the recipe's thresholds.
- **Stars** is the share of crafts that earned exactly 0, 1, 2… stars, adding up to 100%.
- **Scored** is how the deck was sampled: every distinct play order, rolled the same number of times for about `--samples` crafts in all.
- **Cards** is the best deck, with repeated cards counted. The order shown is not a recommended play order.

Once every recipe's best deck is found, they're all sampled together, and the stats for every recipe are printed after the last progress bar finishes. A recipe without enough cards for a deck says so.

## Data files

All game data lives in [RON](https://github.com/ron-rs/ron) files in `data/`, so values can be changed without recompiling. Lines starting with `//` are comments.

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

### `facilities.ron`: what each facility level allows

For each location, three tables keyed by facility level:

- `card_levels`: the highest level each card can be at.
- `card_limits`: the most copies of each card that can go in one deck.
- `recipe_unlocks`: the recipes that unlock at that level, by their names in `recipes.ron`.

```ron
Kitchen: (
	card_levels: {
		8: {HeatControl: 4, Cut: 5, Season: 1, SlowCook: 2, Ferment: 1, Bake: 0},
	},
	card_limits: {
		8: {HeatControl: 4, Cut: 3, Season: 1, SlowCook: 2, Ferment: 2, Bake: 0},
	},
	recipe_unlocks: {
		1: ["Round Bread", "Flatbread"],
		3: ["Toast"],
	},
),
```

Recipes that unlock above your facility level are skipped in `--location` and full runs. A recipe asked for with `--recipe` still runs, with a warning. A recipe that isn't listed counts as unlocked. For now every recipe is listed under level 1, until their unlock levels are known.

`0` means the card isn't unlocked yet at that level. A facility level that isn't listed, or a card that isn't listed for a level, has no known cap, so the command line gives it no copies, and the web page leaves your values for it as they are. That lets the table be filled in a bit at a time. The current data covers levels 1 to 11 (`card_levels`) and 1 to 10 (`card_limits`), from [@MatsuTaku's](https://github.com/MatsuTaku) optimizer, for most cards. The comment at the top of the file lists the cards that aren't covered yet.

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
| [src/inventory.rs](src/inventory.rs) | Your facility levels and cards at each location, and maxing every card for a facility level. |
| [src/facility.rs](src/facility.rs) | The facility level caps in `facilities.ron`. |
| [src/info.rs](src/info.rs) | The Cards and Facilities pages' data: card descriptions, unlock and upgrade levels, values by level, and what unlocks at each facility level. |
| [src/web/app.rs](src/web/app.rs) | The Yew page: location tabs, card inputs, recipe selector, results table and deck builder. |
| [src/builder.rs](src/builder.rs) | The deck builder's slots, and the rules for adding, moving and removing cards. |
| [src/web/worker.rs](src/web/worker.rs), [src/bin/worker.rs](src/bin/worker.rs) | The web worker that scores decks, for both the calculator and the deck builder. |
| [index.html](index.html), [style.css](style.css) | The page shell Trunk builds from, and its styles. |
| [src/deck.rs](src/deck.rs) | Generating decks and play orders, and scoring them. |
| [src/card.rs](src/card.rs) | Every card type and its effect. |
| [src/recipe.rs](src/recipe.rs) | Recipes, locations and recipe powers. |
| [src/score.rs](src/score.rs) | The two-colour score and the modifier stack that cards build up during a craft. |
| [src/tables.rs](src/tables.rs) | Types for the per-level values in `cards.ron`. |

Run the tests with `cargo test`.

## Limitations

- Some card values in `cards.ron` are guesses or placeholders, marked `TODO` or `A guess`. This is because the AFK Journey wiki does not contain this information, nor can you view the previous levels in the game, only the current and next levels. A lot of the values I found prior to my levels are from [@MatsuTaku's](https://github.com/MatsuTaku) [AFK Journey Techcard Optimizer](https://github.com/MatsuTaku/afkjourney-techcard-optimizer), some of which were outdated to do patches.
- Facility level caps are only known up to level 11 and not for every card (see [`facilities.ron`](#facilitiesron-what-each-facility-level-allows)). Above that, or for unlisted cards, use the web page and set each card's level and limit to what your facility currently allows.
- While searching for the best deck, cards with random effects (Heat Control, Cut, random-colour hits) are scored once per distinct play order, so a deck with few distinct orders, such as three copies of one card, gets few random samples. The best deck's reported stats use the samples setting, so they're steadier, but the search itself can still pick a deck that was lucky.

## Roadmap Items

- Update visuals for cards, facilities, and recipes. 
  - The cards will eventually show the full card and the level text for each card/level combo
  - Facilities will hopefully just be higher-res
  - Recipes will also be hopefully higher-res.
- Verify and validate all cards based on in game scoring, some of the card's descriptions are a bit ambiguous to how they actually score, same for recipe powers.
- Hovering over cards should display their abilities