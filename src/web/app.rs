use std::collections::{BTreeMap, VecDeque};

use gloo_storage::{LocalStorage, Storage};
use gloo_worker::{Spawnable, WorkerBridge};
use web_sys::wasm_bindgen::JsCast;
use web_sys::{HtmlElement, HtmlInputElement, HtmlSelectElement, Node};
use yew::html::Scope;
use yew::prelude::*;

use super::data;
use super::worker::{BestDeck, CraftWorker, Job, JobDone, Request, Response, StatsDone, StatsJob};
use crate::builder::{self, Builder, DragSource, DropTarget, Stats};
use crate::card::{Card, CardKind};
use crate::deck::{self, DeckStats};
use crate::facility::{self, Facilities};
use crate::format::{group_digits, star_shares};
use crate::images;
use crate::info;
use crate::inventory::{Facility, Inventory, Owned, available_cards};
use crate::recipe::{Location, Recipe};
use crate::tables::CardTables;

/// Where the player's edited inventory is saved in the browser.
const STORAGE_KEY: &str = "afkjourney-crafting.inventory";
/// Where the deck builder's history is saved in the browser.
const HISTORY_KEY: &str = "afkjourney-crafting.history";
/// Where the deck builder's samples setting is saved in the browser.
const SAMPLES_KEY: &str = "afkjourney-crafting.samples";
/// The deck builder's samples settings: about how many crafts to score per deck.
const SAMPLE_OPTIONS: [(u64, &str); 4] = [
	(10_000, "10,000 (fast)"),
	(50_000, "50,000"),
	(250_000, "250,000"),
	(1_000_000, "1,000,000 (precise)"),
];
const DEFAULT_SAMPLES: u64 = 50_000;
/// Path to the worker script `trunk` builds from `src/bin/worker.rs`.
const WORKER_PATH: &str = "worker.js";
const MAX_WORKERS: usize = 32;
/// What the deck builder shows at a location it hasn't been used at yet.
static EMPTY_BUILDER: Builder = Builder {
	recipe: None,
	slots: Vec::new(),
	stats: None,
};

/// The pages of the app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
	/// Builds a deck by hand and scores it. The page the app opens on.
	Builder,
	/// Finds the best deck for each recipe.
	Calculator,
	/// What each card does, when it unlocks and its values at each level.
	Cards,
	/// What unlocks at each facility level.
	Facilities,
}

impl View {
	/// In the order the sidebar lists them.
	const ALL: [Self; 4] = [Self::Builder, Self::Calculator, Self::Cards, Self::Facilities];

	fn name(self) -> &'static str {
		match self {
			Self::Builder => "Deck builder",
			Self::Calculator => "Best decks",
			Self::Cards => "Cards",
			Self::Facilities => "Facilities",
		}
	}

	/// What the page is for, under its name in the sidebar.
	fn blurb(self) -> &'static str {
		match self {
			Self::Builder => "Build and score your own deck",
			Self::Calculator => "Find the best deck for each recipe",
			Self::Cards => "What each card does at each level",
			Self::Facilities => "What unlocks at each facility level",
		}
	}

	/// A small line icon for the sidebar, drawn in the text colour.
	fn icon(self) -> Html {
		let paths = match self {
			// Two overlapping cards
			Self::Builder => html! {
				<>
					<rect x="3" y="6" width="11" height="15" rx="2" />
					<path d="M8 6V5a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v13a2 2 0 0 1-2 2h-5" />
				</>
			},
			// A trophy
			Self::Calculator => html! {
				<>
					<path d="M7 4h10v5a5 5 0 0 1-10 0z" />
					<path d="M7 6H4v1a3 3 0 0 0 3 3M17 6h3v1a3 3 0 0 1-3 3" />
					<path d="M12 14v4M8 21h8M9 18h6" />
				</>
			},
			// An open book
			Self::Cards => html! {
				<>
					<path d="M2 5h6a4 4 0 0 1 4 4v11a3 3 0 0 0-3-3H2z" />
					<path d="M22 5h-6a4 4 0 0 0-4 4v11a3 3 0 0 1 3-3h7z" />
				</>
			},
			// A building with a door
			Self::Facilities => html! {
				<>
					<path d="M3 21h18M5 21V10l7-6 7 6v11" />
					<path d="M10 21v-5h4v5" />
				</>
			},
		};
		html! {
			<svg
				class="nav-icon"
				viewBox="0 0 24 24"
				fill="none"
				stroke="currentColor"
				stroke-width="1.8"
				stroke-linecap="round"
				stroke-linejoin="round"
				aria-hidden="true"
			>
				{ paths }
			</svg>
		}
	}
}

pub enum Msg {
	SetView(View),
	SelectLocation(Location),
	/// `None` leaves the location's cards uncapped.
	SetFacilityLevel(Option<u8>),
	/// Sets every card at the location to the most its facility level allows.
	UseMaximums,
	SetLevel(CardKind, u8),
	SetLimit(CardKind, usize),
	ResetInventory,
	/// `None` selects every recipe at the current location.
	SelectRecipe(Option<usize>),
	Calculate,
	Cancel,
	/// A calculator run's worker finished a search job or sampled a best deck.
	RunResponse {
		run: u32,
		worker: usize,
		response: Response,
	},
	/// `None` clears the deck builder's recipe.
	BuilderRecipe(Option<usize>),
	/// Puts a card in the deck builder's first empty slot.
	BuilderAdd(CardKind),
	/// Takes the card out of a deck builder slot.
	BuilderRemove(usize),
	BuilderClear,
	DragStart(DragSource),
	DragOver(DropTarget),
	DragLeave(DropTarget),
	DropCard(DropTarget),
	DragEnd,
	/// Scores every play order of the deck builder's deck.
	ScoreDeck,
	StatsDone(StatsDone),
	/// Finds the best deck for the deck builder's recipe, to compare with.
	FindBest,
	StopFindBest,
	SearchJobDone {
		search: u32,
		worker: usize,
		done: JobDone,
	},
	/// Puts a deck from the recipe's history back in the deck builder.
	LoadDeck(usize),
	RemoveHistory(usize),
	ClearHistory,
	/// About how many crafts to score per deck in the deck builder.
	SetSamples(u64),
	/// Scores the best deck for the deck builder's recipe with the samples setting.
	ScoreBest,
}

/// The best deck found for a recipe, from either the calculator or the deck builder.
struct KnownBest {
	/// The cards it was found from. It's only the best deck while these are still
	/// the location's available cards.
	cards: Vec<(Card, usize)>,
	/// `None` when there weren't enough cards for the recipe.
	best: Option<BestDeck>,
}

/// The best deck for a recipe, scored the way the deck builder scores decks, so
/// the two compare like for like.
struct BestStats {
	/// The best deck's cards, sorted.
	cards: Vec<Card>,
	/// The samples setting it was scored with.
	samples: u64,
	/// `None` while it's being scored.
	result: Option<DeckStats>,
	/// Being scored as the deck builder's deck, which is the same deck, rather
	/// than by its own worker.
	from_builder: bool,
}

/// The deck builder's search for the best deck for one recipe, to compare with.
struct Search {
	recipe: usize,
	run: Run,
	jobs_left: usize,
	best: Option<BestDeck>,
}

/// The result for one recipe, filled in as its jobs finish.
struct RecipeResult {
	recipe: usize,
	jobs_left: usize,
	best: Option<BestDeck>,
	/// The samples setting the best deck is scored with once it's found.
	samples: u64,
	/// The best deck scored like the deck builder's decks, once that's done.
	stats: Option<DeckStats>,
	/// Whether the best deck is being scored.
	sampling: bool,
}

impl RecipeResult {
	/// Whether the search and the scoring of the best deck are both done.
	fn done(&self) -> bool {
		self.jobs_left == 0 && !self.sampling
	}
}

/// The workers and queued jobs of a best deck search in progress.
struct Run {
	location: Location,
	/// The cards the decks are built from.
	cards: Vec<(Card, usize)>,
	workers: Vec<WorkerBridge<CraftWorker>>,
	idle: Vec<usize>,
	/// Search jobs, and best decks to score.
	queue: VecDeque<Request>,
}

impl Run {
	/// Spawns one worker per CPU core and starts finding the best deck for each of
	/// `recipes`, split into one job per worker. `on_done` makes the message, if
	/// any, for a worker's response, from the worker's number and the response.
	fn start(
		link: &Scope<App>,
		location: Location,
		cards: Vec<(Card, usize)>,
		recipes: &[usize],
		on_done: impl Fn(usize, Response) -> Option<Msg> + Clone + 'static,
	) -> Self {
		let parts = worker_count();
		let workers = (0..parts)
			.map(|worker| {
				let link = link.clone();
				let on_done = on_done.clone();
				CraftWorker::spawner()
					.callback(move |response| {
						if let Some(msg) = on_done(worker, response) {
							link.send_message(msg);
						}
					})
					.spawn(WORKER_PATH)
			})
			.collect();
		let queue = recipes
			.iter()
			.flat_map(|&recipe| {
				let cards = &cards;
				(0..parts).map(move |part| {
					Request::Best(Job {
						recipe,
						cards: cards.clone(),
						part,
						parts,
					})
				})
			})
			.collect();
		let mut run = Self {
			location,
			cards,
			workers,
			idle: (0..parts).rev().collect(),
			queue,
		};
		run.dispatch();
		run
	}

	/// How many jobs each recipe is split into.
	fn parts(&self) -> usize {
		self.workers.len()
	}

	/// Hands queued jobs to idle workers.
	fn dispatch(&mut self) {
		while let Some(&worker) = self.idle.last() {
			let Some(request) = self.queue.pop_front() else { break };
			self.idle.pop();
			self.workers[worker].send(request);
		}
	}
}

pub struct App {
	recipes: Vec<Recipe>,
	facilities: Facilities,
	/// Card values by level, for the card guide.
	tables: CardTables,
	inventory: Inventory,
	location: Location,
	selected: Option<usize>,
	/// The latest results at each location.
	results: BTreeMap<Location, Vec<RecipeResult>>,
	run: Option<Run>,
	/// Increases with every run, so results from a cancelled run are ignored.
	run_id: u32,
	view: View,
	/// The deck builder at each location.
	builders: BTreeMap<Location, Builder>,
	/// The card being dragged in the deck builder.
	dragging: Option<DragSource>,
	/// Where the dragged card would land if dropped now.
	drop_target: Option<DropTarget>,
	/// Scores the deck builder's deck. Replacing it cancels the deck it was scoring.
	stats_worker: Option<WorkerBridge<CraftWorker>>,
	/// The best deck found for each recipe.
	best_decks: BTreeMap<usize, KnownBest>,
	/// The deck builder's best deck search in progress.
	search: Option<Search>,
	/// Increases with every search, so results from a stopped search are ignored.
	search_id: u32,
	/// The decks scored in the deck builder, newest first, by recipe name.
	history: BTreeMap<String, Vec<DeckStats>>,
	/// About how many crafts the deck builder scores per deck.
	samples: u64,
	/// The best deck for each recipe, scored like the deck builder's decks.
	best_stats: BTreeMap<usize, BestStats>,
	/// Scores the best deck. Replacing it cancels the deck it was scoring.
	best_stats_worker: Option<WorkerBridge<CraftWorker>>,
}

impl Component for App {
	type Message = Msg;
	type Properties = ();

	fn create(_ctx: &Context<Self>) -> Self {
		Self {
			recipes: data::recipes(),
			facilities: data::facilities(),
			tables: data::tables(),
			inventory: load_inventory(),
			location: Location::Kitchen,
			selected: None,
			results: BTreeMap::new(),
			run: None,
			run_id: 0,
			view: View::Builder,
			builders: BTreeMap::new(),
			dragging: None,
			drop_target: None,
			stats_worker: None,
			best_decks: BTreeMap::new(),
			search: None,
			search_id: 0,
			history: load_history(),
			samples: LocalStorage::get::<u64>(SAMPLES_KEY)
				.ok()
				.filter(|samples| SAMPLE_OPTIONS.iter().any(|&(option, _)| option == *samples))
				.unwrap_or(DEFAULT_SAMPLES),
			best_stats: BTreeMap::new(),
			best_stats_worker: None,
		}
	}

	fn update(&mut self, ctx: &Context<Self>, msg: Msg) -> bool {
		match msg {
			Msg::SetView(view) => self.view = view,
			Msg::SelectLocation(location) => {
				if location != self.location {
					self.location = location;
					self.selected = None;
				}
			}
			Msg::SetFacilityLevel(level) => {
				self.inventory.entry(self.location).or_default().level = level;
				save_inventory(&self.inventory);
				// A recipe the new level hasn't unlocked can't stay selected
				if self.selected.is_some_and(|selected| !self.location_recipes().any(|i| i == selected)) {
					self.selected = None;
				}
				self.fit_builders();
			}
			Msg::UseMaximums => {
				let location = self.location;
				let facility = self.inventory.entry(location).or_default();
				for kind in CardKind::ALL.into_iter().filter(|kind| kind.location() == location) {
					let maximum = facility.cap(&self.facilities, location, kind).maximum(facility.owned(kind));
					facility.cards.insert(kind, maximum);
				}
				save_inventory(&self.inventory);
				self.fit_builders();
			}
			Msg::SetLevel(kind, level) => {
				self.owned_mut(kind).level = level;
				save_inventory(&self.inventory);
			}
			Msg::SetLimit(kind, limit) => {
				self.owned_mut(kind).limit = limit;
				save_inventory(&self.inventory);
				self.fit_builders();
			}
			Msg::ResetInventory => {
				self.inventory = data::default_inventory();
				LocalStorage::delete(STORAGE_KEY);
				self.fit_builders();
			}
			Msg::SelectRecipe(recipe) => self.selected = recipe,
			Msg::Calculate => self.start_run(ctx),
			Msg::Cancel => {
				self.run = None;
				self.run_id += 1;
			}
			Msg::RunResponse { run, worker, response } => {
				if run != self.run_id {
					return false;
				}
				self.run_response(worker, response);
			}
			Msg::BuilderRecipe(recipe) => {
				let size = recipe.map_or(0, |recipe| self.recipes[recipe].cards);
				self.builder_mut().set_recipe(recipe, size);
			}
			Msg::BuilderAdd(kind) => {
				let available = self.available();
				self.builder_mut().add(&available, kind);
			}
			Msg::BuilderRemove(slot) => self.builder_mut().remove(slot),
			Msg::BuilderClear => self.builder_mut().clear(),
			Msg::DragStart(source) => self.dragging = Some(source),
			Msg::DragOver(target) => {
				// Fires many times a second while hovering, so only redraw on a change
				if self.dragging.is_none() || self.drop_target == Some(target) {
					return false;
				}
				self.drop_target = Some(target);
			}
			Msg::DragLeave(target) => {
				if self.drop_target != Some(target) {
					return false;
				}
				self.drop_target = None;
			}
			Msg::DropCard(target) => {
				if let Some(source) = self.dragging.take() {
					let available = self.available();
					self.builder_mut().drop(&available, source, target);
				}
				self.drop_target = None;
			}
			Msg::DragEnd => {
				self.dragging = None;
				self.drop_target = None;
			}
			Msg::ScoreDeck => {
				if !self.builder().is_full() {
					return false;
				}
				self.score_deck(ctx);
				// Something to compare the deck with
				self.find_best(ctx);
				self.score_best(ctx);
			}
			Msg::ScoreBest => self.score_best(ctx),
			Msg::StatsDone(done) => self.stats_done(done),
			Msg::FindBest => self.find_best(ctx),
			Msg::StopFindBest => {
				self.search = None;
				self.search_id += 1;
			}
			Msg::SearchJobDone { search, worker, done } => {
				if search != self.search_id {
					return false;
				}
				self.search_job_done(worker, done);
				if self.search.is_none() {
					// Just found: score it like the builder's deck
					self.score_best(ctx);
				}
			}
			Msg::LoadDeck(index) => {
				let Some(recipe) = self.builder().recipe else { return false };
				let Some(entry) = self.history_for(recipe).get(index).cloned() else { return false };
				let available = self.available();
				let builder = self.builder_mut();
				builder.load(&available, &entry.cards);
				// Its stats still hold if every card is at the same level
				if builder.deck(&available).as_ref() == Some(&entry.cards) {
					builder.stats = Some(Stats {
						recipe,
						cards: entry.cards.clone(),
						result: Some(entry),
					});
				}
			}
			Msg::RemoveHistory(index) => {
				let Some(recipe) = self.builder().recipe else { return false };
				if let Some(entries) = self.history.get_mut(&self.recipes[recipe].name)
					&& index < entries.len()
				{
					entries.remove(index);
					save_history(&self.history);
				}
			}
			Msg::SetSamples(samples) => {
				self.samples = samples;
				// Not being able to save only loses the setting on reload
				let _ = LocalStorage::set(SAMPLES_KEY, samples);
			}
			Msg::ClearHistory => {
				let Some(recipe) = self.builder().recipe else { return false };
				self.history.remove(&self.recipes[recipe].name);
				save_history(&self.history);
			}
		}
		true
	}

	fn view(&self, ctx: &Context<Self>) -> Html {
		let link = ctx.link();
		let wide = (self.view == View::Builder).then_some("wide");
		html! {
			<div class="app">
				<nav class="sidebar" aria-label="Pages">
					<div class="brand">{ "Homestead Crafting Calculator" }</div>
					<ul class="nav-list">
						{ for View::ALL.into_iter().map(|view| {
							let active = view == self.view;
							html! {
								<li>
									<button
										class={classes!("nav-item", active.then_some("active"))}
										aria-current={active.then_some("page")}
										onclick={link.callback(move |_| Msg::SetView(view))}
									>
										{ view.icon() }
										<span class="nav-text">
											<span class="nav-name">{ view.name() }</span>
											<span class="nav-blurb">{ view.blurb() }</span>
										</span>
									</button>
								</li>
							}
						}) }
					</ul>
				</nav>
				<div class="app-body">
				<header class={classes!("app-header", wide)}>
					<h1>{ self.view.name() }</h1>
					<nav class="tabs" role="tablist" aria-label="Location">
						{ for Location::ALL.iter().map(|&location| {
							let active = location == self.location;
							html! {
								<button
									role="tab"
									aria-selected={active.to_string()}
									class={classes!("tab", active.then_some("active"))}
									onclick={link.callback(move |_| Msg::SelectLocation(location))}
								>
									{ image(images::facility(location), "tab-icon") }
									{ location.name() }
								</button>
							}
						}) }
					</nav>
				</header>
				<main class={classes!(wide)}>
					if self.view == View::Builder {
						<div class="builder-layout">
							<div class="builder-main">
								{ self.view_inventory(ctx) }
								{ self.view_builder(ctx) }
							</div>
							{ self.view_history(link) }
						</div>
					} else if self.view == View::Calculator {
						{ self.view_inventory(ctx) }
						{ self.view_controls(ctx) }
						{ self.view_results() }
					} else if self.view == View::Cards {
						{ self.view_card_guide() }
					} else {
						{ self.view_facility_guide() }
					}
				</main>
				</div>
			</div>
		}
	}
}

impl App {
	fn owned_mut(&mut self, kind: CardKind) -> &mut Owned {
		self.inventory.entry(kind.location()).or_default().cards.entry(kind).or_default()
	}

	fn facility(&self) -> Facility {
		self.inventory.get(&self.location).cloned().unwrap_or_default()
	}

	/// Indices of the recipes made at the current location that its facility level has unlocked.
	fn location_recipes(&self) -> impl Iterator<Item = usize> + '_ {
		(0..self.recipes.len())
			.filter(move |&i| self.recipes[i].location == self.location && self.recipe_unlocked(i))
	}

	/// Whether the facility level at the recipe's location has unlocked it.
	fn recipe_unlocked(&self, recipe: usize) -> bool {
		let recipe = &self.recipes[recipe];
		self.inventory
			.get(&recipe.location)
			.is_none_or(|facility| facility.recipe_unlocked(&self.facilities, recipe))
	}

	/// The cards that may be used at the current location, with how many copies of each.
	fn available(&self) -> Vec<(Card, usize)> {
		available_cards(&self.inventory, &self.facilities, self.location)
	}

	fn builder(&self) -> &Builder {
		self.builders.get(&self.location).unwrap_or(&EMPTY_BUILDER)
	}

	fn builder_mut(&mut self) -> &mut Builder {
		self.builders.entry(self.location).or_default()
	}

	/// Keeps every deck builder to what the inventory and facility levels allow,
	/// after they change.
	fn fit_builders(&mut self) {
		for location in Location::ALL {
			let available = available_cards(&self.inventory, &self.facilities, location);
			let locked = self
				.builders
				.get(&location)
				.and_then(|builder| builder.recipe)
				.is_some_and(|recipe| !self.recipe_unlocked(recipe));
			let Some(builder) = self.builders.get_mut(&location) else { continue };
			if locked {
				builder.set_recipe(None, 0);
			}
			builder.fit(&available);
		}
	}

	/// Starts scoring the deck builder's deck in a worker.
	fn score_deck(&mut self, ctx: &Context<Self>) {
		let available = self.available();
		let builder = self.builder();
		let (Some(recipe), Some(cards)) = (builder.recipe, builder.deck(&available)) else {
			return;
		};
		if builder.current_stats(&available).is_some_and(|stats| stats.result.is_none()) {
			// Already being scored
			return;
		}

		// The new worker replaces the old one, which cancels any deck it was scoring
		for builder in self.builders.values_mut() {
			if builder.stats.as_ref().is_some_and(|stats| stats.result.is_none()) {
				builder.stats = None;
			}
		}
		// Including a best deck that was waiting on it
		self.best_stats.retain(|_, best| best.result.is_some() || !best.from_builder);
		let link = ctx.link().clone();
		let worker = CraftWorker::spawner()
			.callback(move |response| {
				if let Response::Stats(done) = response {
					link.send_message(Msg::StatsDone(done));
				}
			})
			.spawn(WORKER_PATH);
		worker.send(Request::Stats(StatsJob {
			samples: self.samples,
			recipe,
			cards: cards.clone(),
		}));
		self.stats_worker = Some(worker);
		self.builder_mut().stats = Some(Stats {
			recipe,
			cards,
			result: None,
		});
	}

	fn stats_done(&mut self, done: StatsDone) {
		// Either the best deck, the builder's deck, or both when they're the same deck
		if let Some(best) = self.best_stats.get_mut(&done.recipe)
			&& best.result.is_none()
			&& best.cards == done.stats.cards
		{
			best.result = Some(done.stats.clone());
			self.best_stats_worker = None;
		}
		let location = self.recipes[done.recipe].location;
		let Some(stats) = self.builders.get_mut(&location).and_then(|builder| builder.stats.as_mut()) else {
			return;
		};
		if stats.recipe == done.recipe && stats.cards == done.stats.cards && stats.result.is_none() {
			let name = self.recipes[done.recipe].name.clone();
			builder::record(self.history.entry(name).or_default(), done.stats.clone());
			save_history(&self.history);
			stats.result = Some(done.stats);
			// Done with the worker
			self.stats_worker = None;
		}
	}

	/// The best deck for `recipe` scored with the current samples setting, or being
	/// scored, while it's still the best deck for the cards available now.
	fn best_stats_for(&self, recipe: usize) -> Option<&BestStats> {
		let cards = &self.known_best(recipe)?.best.as_ref()?.cards;
		self.best_stats.get(&recipe).filter(|stats| &stats.cards == cards && stats.samples == self.samples)
	}

	/// The best deck's average for `recipe`: from scoring it with the samples
	/// setting once that's done, otherwise from the search, which scores each
	/// play order once.
	fn best_average(&self, recipe: usize) -> Option<i64> {
		let sampled = self.best_stats_for(recipe).and_then(|stats| stats.result.as_ref());
		sampled.map(|stats| stats.average).or_else(|| Some(self.known_best(recipe)?.best.as_ref()?.average))
	}

	/// Scores the best deck for the deck builder's recipe with the samples setting,
	/// once it's known, unless that's done or underway.
	fn score_best(&mut self, ctx: &Context<Self>) {
		let Some(recipe) = self.builder().recipe else { return };
		let Some(cards) = self.known_best(recipe).and_then(|known| known.best.as_ref()).map(|best| best.cards.clone())
		else {
			return;
		};
		if self.best_stats_for(recipe).is_some() {
			return;
		}
		let samples = self.samples;
		// The builder's deck may be the best deck, already scored the same way, or
		// being scored now (with the current setting), which fills this in too
		let available = self.available();
		let current = self.builder().current_stats(&available).filter(|stats| stats.cards == cards);
		let scoring = current.is_some_and(|stats| stats.result.is_none());
		let scored = current
			.and_then(|stats| stats.result.clone())
			.filter(|stats| stats.rolls == deck::rolls_per_order(stats.orders, samples));
		if scored.is_none() && !scoring {
			// The new worker replaces the old one, which cancels any deck it was scoring
			self.best_stats.retain(|_, stats| stats.result.is_some());
			let link = ctx.link().clone();
			let worker = CraftWorker::spawner()
				.callback(move |response| {
					if let Response::Stats(done) = response {
						link.send_message(Msg::StatsDone(done));
					}
				})
				.spawn(WORKER_PATH);
			worker.send(Request::Stats(StatsJob {
				recipe,
				cards: cards.clone(),
				samples,
			}));
			self.best_stats_worker = Some(worker);
		}
		self.best_stats.insert(recipe, BestStats {
			cards,
			samples,
			result: scored,
			from_builder: scoring,
		});
	}

	/// The decks scored for `recipe`, newest first.
	fn history_for(&self, recipe: usize) -> &[DeckStats] {
		self.history.get(&self.recipes[recipe].name).map(Vec::as_slice).unwrap_or_default()
	}

	/// The best deck for `recipe` from the cards available now, if it's been found.
	fn known_best(&self, recipe: usize) -> Option<&KnownBest> {
		let cards = available_cards(&self.inventory, &self.facilities, self.recipes[recipe].location);
		self.best_decks.get(&recipe).filter(|known| known.cards == cards)
	}

	/// The search in progress for `recipe`'s best deck from the cards available now.
	fn current_search(&self, recipe: usize) -> Option<&Search> {
		let cards = available_cards(&self.inventory, &self.facilities, self.recipes[recipe].location);
		self.search.as_ref().filter(|search| search.recipe == recipe && search.run.cards == cards)
	}

	/// Starts finding the best deck for the deck builder's recipe, unless it's
	/// already known or being found. Stops any search for another recipe.
	fn find_best(&mut self, ctx: &Context<Self>) {
		let Some(recipe) = self.builder().recipe else { return };
		if self.known_best(recipe).is_some() || self.current_search(recipe).is_some() {
			return;
		}
		self.search_id += 1;
		let search_id = self.search_id;
		let run = Run::start(ctx.link(), self.location, self.available(), &[recipe], move |worker, response| {
			match response {
				Response::Best(done) => Some(Msg::SearchJobDone { search: search_id, worker, done }),
				Response::Stats(_) => None,
			}
		});
		self.search = Some(Search {
			recipe,
			jobs_left: run.parts(),
			run,
			best: None,
		});
	}

	fn search_job_done(&mut self, worker: usize, done: JobDone) {
		let Some(search) = &mut self.search else { return };
		search.jobs_left = search.jobs_left.saturating_sub(1);
		if let Some(best) = done.best
			&& search.best.as_ref().is_none_or(|current| best.beats(current))
		{
			search.best = Some(best);
		}
		search.run.idle.push(worker);
		search.run.dispatch();
		if search.jobs_left == 0
			&& let Some(search) = self.search.take()
		{
			// Dropping the run shuts its workers down
			self.best_decks.insert(search.recipe, KnownBest {
				cards: search.run.cards,
				best: search.best,
			});
		}
	}

	/// How many recipes at the current location aren't unlocked yet.
	fn locked_recipe_count(&self) -> usize {
		let total = self.recipes.iter().filter(|recipe| recipe.location == self.location).count();
		total - self.location_recipes().count()
	}

	fn start_run(&mut self, ctx: &Context<Self>) {
		if self.run.is_some() {
			return;
		}
		let recipes: Vec<usize> = match self.selected {
			Some(recipe) => vec![recipe],
			None => self.location_recipes().collect(),
		};
		self.run_id += 1;
		let run_id = self.run_id;
		let run = Run::start(ctx.link(), self.location, self.available(), &recipes, move |worker, response| {
			Some(Msg::RunResponse { run: run_id, worker, response })
		});
		self.results.insert(
			self.location,
			recipes
				.iter()
				.map(|&recipe| RecipeResult {
					recipe,
					jobs_left: run.parts(),
					best: None,
					samples: self.samples,
					stats: None,
					sampling: false,
				})
				.collect(),
		);
		self.run = Some(run);
	}

	fn run_response(&mut self, worker: usize, response: Response) {
		let Some(run) = &mut self.run else { return };
		let rows = self.results.entry(run.location).or_default();
		match response {
			Response::Best(done) => {
				if let Some(row) = rows.iter_mut().find(|row| row.recipe == done.recipe) {
					row.jobs_left = row.jobs_left.saturating_sub(1);
					if let Some(best) = done.best
						&& row.best.as_ref().is_none_or(|current| best.beats(current))
					{
						row.best = Some(best);
					}
					if row.jobs_left == 0 {
						// Lets the deck builder compare against it
						self.best_decks.insert(row.recipe, KnownBest {
							cards: run.cards.clone(),
							best: row.best.clone(),
						});
						if let Some(best) = &row.best {
							// Score the best deck like the deck builder does: the builder
							// may already have, otherwise it's next in line
							let sampled = self
								.best_stats
								.get(&row.recipe)
								.filter(|stats| stats.cards == best.cards && stats.samples == row.samples)
								.and_then(|stats| stats.result.clone());
							if sampled.is_some() {
								row.stats = sampled;
							} else {
								row.sampling = true;
								run.queue.push_front(Request::Stats(StatsJob {
									recipe: row.recipe,
									cards: best.cards.clone(),
									samples: row.samples,
								}));
							}
						}
					}
				}
			}
			Response::Stats(done) => {
				if let Some(row) = rows.iter_mut().find(|row| row.recipe == done.recipe && row.sampling) {
					row.sampling = false;
					// Lets the deck builder use it too
					self.best_stats.insert(row.recipe, BestStats {
						cards: done.stats.cards.clone(),
						samples: row.samples,
						result: Some(done.stats.clone()),
						from_builder: false,
					});
					row.stats = Some(done.stats);
				}
			}
		}

		run.idle.push(worker);
		run.dispatch();
		if rows.iter().all(RecipeResult::done) {
			// Dropping the run shuts its workers down
			self.run = None;
		}
	}

	fn view_inventory(&self, ctx: &Context<Self>) -> Html {
		let link = ctx.link();
		let location = self.location;
		let facility = self.facility();
		let kinds = CardKind::ALL.into_iter().filter(|kind| kind.location() == location);

		let on_facility_level = link.callback(|e: Event| {
			let value = e.target_unchecked_into::<HtmlSelectElement>().value();
			Msg::SetFacilityLevel(value.parse().ok())
		});
		let level_hint = match facility.level {
			None => "Set the facility level to cap each card to what that level allows.".to_string(),
			Some(level) if !self.has_cap_data(level) => {
				format!("No data for {} level {level} yet, so cards aren't capped.", location.name())
			}
			Some(level) => format!("Cards are capped to what {} level {level} allows.", location.name()),
		};

		html! {
			<section class="panel" aria-labelledby="inventory-heading">
				<div class="panel-header">
					<div class="heading-with-image">
						{ image(images::facility(location), "facility-image") }
						<h2 id="inventory-heading">{ format!("{} cards", location.name()) }</h2>
					</div>
					<button class="button subtle" onclick={link.callback(|_| Msg::ResetInventory)}>
						{ "Reset all locations to defaults" }
					</button>
				</div>
				<div class="facility-row">
					<label class="facility-level">
						<span>{ "Facility level" }</span>
						<select onchange={on_facility_level}>
							<option value="" selected={facility.level.is_none()}>{ "Not set" }</option>
							{ for (1..=facility::MAX_LEVEL).map(|level| html! {
								<option value={level.to_string()} selected={facility.level == Some(level)}>
									{ level }
								</option>
							}) }
						</select>
					</label>
					<button
						class="button subtle"
						disabled={facility.level.is_none()}
						title="Set every card to the highest level and copies this facility level allows"
						onclick={link.callback(|_| Msg::UseMaximums)}
					>
						{ "Use level maximums" }
					</button>
				</div>
				<p class="hint">
					{ level_hint }
					{ " A limit of 0 leaves the card out. Changes are saved in this browser." }
				</p>
				<div class="card-grid">
					{ for kinds.map(|kind| self.view_card_input(ctx, &facility, kind)) }
				</div>
			</section>
		}
	}

	/// Whether the facilities data has any caps for `level` at the current location.
	fn has_cap_data(&self, level: u8) -> bool {
		self.facilities.0.get(&self.location).is_some_and(|table| {
			table.card_levels.contains_key(&level) || table.card_limits.contains_key(&level)
		})
	}

	fn view_card_input(&self, ctx: &Context<Self>, facility: &Facility, kind: CardKind) -> Html {
		let link = ctx.link();
		let owned = facility.owned(kind);
		let cap = facility.cap(&self.facilities, self.location, kind);
		let locked = cap.locked();
		let used = cap.apply(owned);

		let on_level = link.batch_callback(move |e: InputEvent| {
			let value = e.target_unchecked_into::<HtmlInputElement>().value();
			value.parse::<u8>().ok().filter(|&level| level >= 1).map(|level| Msg::SetLevel(kind, level))
		});
		let on_limit = link.batch_callback(move |e: InputEvent| {
			let value = e.target_unchecked_into::<HtmlInputElement>().value();
			value.parse::<usize>().ok().filter(|&limit| limit <= 99).map(|limit| Msg::SetLimit(kind, limit))
		});

		let note = if locked {
			Some(format!("Not unlocked at level {}", facility.level.unwrap_or_default()))
		} else if cap != facility::Cap::default() {
			let level = cap.level.map_or("?".to_string(), |level| level.to_string());
			let copies = match cap.limit {
				Some(1) => "1 copy".to_string(),
				Some(limit) => format!("{limit} copies"),
				None => "? copies".to_string(),
			};
			Some(format!("Max Lv {level}, {copies}"))
		} else {
			None
		};
		let level_over = !locked && used.level < owned.level;
		let limit_over = !locked && used.limit < owned.limit;

		html! {
			<div class={classes!("card-input", (used.limit == 0).then_some("unused"), locked.then_some("locked"))}>
				<div class="card-label">
					{ image(images::card(kind), "card-image") }
					<div class="card-text">
						<span class="card-name">{ kind.name() }</span>
						if let Some(note) = note {
							<span class="card-note">{ note }</span>
						}
					</div>
				</div>
				<label class={classes!(level_over.then_some("over"))}>
					<span>{ if level_over { format!("Level → {}", used.level) } else { "Level".to_string() } }</span>
					<input
						type="number"
						min="1"
						max={cap.level.map_or("255".to_string(), |level| level.max(1).to_string())}
						value={owned.level.to_string()}
						disabled={locked}
						oninput={on_level}
					/>
				</label>
				<label class={classes!(limit_over.then_some("over"))}>
					<span>{ if limit_over { format!("Limit → {}", used.limit) } else { "Limit".to_string() } }</span>
					<input
						type="number"
						min="0"
						max={cap.limit.map_or("99".to_string(), |limit| limit.to_string())}
						value={owned.limit.to_string()}
						disabled={locked}
						oninput={on_limit}
					/>
				</label>
			</div>
		}
	}

	fn view_controls(&self, ctx: &Context<Self>) -> Html {
		let link = ctx.link();
		let on_select = link.callback(|e: Event| {
			let value = e.target_unchecked_into::<HtmlSelectElement>().value();
			Msg::SelectRecipe(value.parse().ok())
		});
		let running_here = self.run.as_ref().map(|run| run.location == self.location);

		html! {
			<section class="controls">
				<label class="recipe-select">
					<span>{ "Recipe" }</span>
					<select onchange={on_select}>
						<option value="" selected={self.selected.is_none()}>{ "All" }</option>
						{ for self.location_recipes().map(|i| html! {
							<option value={i.to_string()} selected={self.selected == Some(i)}>
								{ &self.recipes[i].name }
							</option>
						}) }
					</select>
				</label>
				if self.run.is_some() {
					<button class="button" onclick={link.callback(|_| Msg::Cancel)}>{ "Cancel" }</button>
				} else {
					<button class="button primary" onclick={link.callback(|_| Msg::Calculate)}>{ "Calculate" }</button>
				}
				{ self.view_samples(link, self.run.is_some()) }
				{ match running_here {
					Some(true) => html! { <span class="status">{ self.progress_text() }</span> },
					Some(false) => html! {
						<span class="status">
							{ format!("Calculating {} recipes…", self.run.as_ref().map_or("", |run| run.location.name())) }
						</span>
					},
					None => match self.locked_recipe_count() {
						0 => html! {},
						1 => html! { <span class="status">{ "1 more recipe unlocks at a higher facility level" }</span> },
						locked => html! {
							<span class="status">{ format!("{locked} more recipes unlock at higher facility levels") }</span>
						},
					},
				} }
			</section>
		}
	}

	fn progress_text(&self) -> String {
		let rows = self.results.get(&self.location).map(Vec::as_slice).unwrap_or_default();
		let done = rows.iter().filter(|row| row.done()).count();
		format!("Calculating… {done} of {} done", rows.len())
	}

	fn view_results(&self) -> Html {
		let Some(rows) = self.results.get(&self.location).filter(|rows| !rows.is_empty()) else {
			return html! {};
		};
		let running = self.run.as_ref().is_some_and(|run| run.location == self.location);

		html! {
			<section class="panel" aria-labelledby="results-heading">
				<h2 id="results-heading">{ "Best decks" }</h2>
				<p class="hint">
					{ "The deck with the highest average score over every play order. Once found, it's scored again with the samples setting for a steadier average and its chance of each star. Card order is not a play order." }
				</p>
				<div class="table-wrap">
					<table class="results">
						<thead>
							<tr><th scope="col">{ "Recipe" }</th><th scope="col">{ "Cards" }</th></tr>
						</thead>
						<tbody>
							{ for rows.iter().map(|row| self.view_row(row, running)) }
						</tbody>
					</table>
				</div>
			</section>
		}
	}

	fn view_row(&self, row: &RecipeResult, running: bool) -> Html {
		let recipe = &self.recipes[row.recipe];
		let best = row.best.as_ref().filter(|_| row.jobs_left == 0);
		let cards = if row.jobs_left > 0 {
			let text = if running { "Calculating…" } else { "Cancelled" };
			html! { <span class="muted">{ text }</span> }
		} else {
			match best {
				Some(best) => html! {
					<>
						{ view_cards(&best.cards) }
						if let Some(stats) = &row.stats {
							{ view_star_distribution(recipe, stats) }
							<div class="recipe-summary">
								{ format!(
									"Low {} · High {} · {} crafts",
									stats.minimum.total(),
									stats.maximum.total(),
									group_digits(stats.crafts()),
								) }
							</div>
						} else if row.sampling && running {
							<div class="recipe-summary">{ "Scoring its chance of each star…" }</div>
						}
					</>
				},
				None => html! { <span class="muted">{ format!("Not enough cards: needs {}", recipe.cards) }</span> },
			}
		};
		// The sampled average once it's in, otherwise the search's
		let summary = best.map(|best| {
			let average = row.stats.as_ref().map_or(best.average, |stats| stats.average);
			html! {
				<div class="recipe-summary">
					{ format!("Avg {average} ") }
					{ view_stars(recipe, average) }
				</div>
			}
		});

		html! {
			<tr>
				<th scope="row">
					<div class="recipe-heading">
						{ image(images::recipe(recipe), "recipe-image") }
						<div>
							<div>{ &recipe.name }</div>
							{ for summary }
						</div>
					</div>
				</th>
				<td>{ cards }</td>
			</tr>
		}
	}

	/// The samples setting: about how many crafts to score per deck, for both the
	/// best deck calculator and the deck builder.
	fn view_samples(&self, link: &Scope<Self>, disabled: bool) -> Html {
		html! {
			<label class="samples" title="About how many crafts to score each deck with. Each play order is scored the same number of times, and at least once, so decks with fewer play orders get more rolls of each.">
				<span>{ "Samples" }</span>
				<select
					{disabled}
					onchange={link.batch_callback(|e: Event| {
						e.target_unchecked_into::<HtmlSelectElement>().value().parse().ok().map(Msg::SetSamples)
					})}
				>
					{ for SAMPLE_OPTIONS.iter().map(|&(samples, label)| html! {
						<option value={samples.to_string()} selected={samples == self.samples}>{ label }</option>
					}) }
				</select>
			</label>
		}
	}
}

/// The deck builder.
impl App {
	fn view_builder(&self, ctx: &Context<Self>) -> Html {
		let builder = self.builder();
		let on_recipe = ctx.link().callback(|e: Event| {
			let value = e.target_unchecked_into::<HtmlSelectElement>().value();
			Msg::BuilderRecipe(value.parse().ok())
		});

		html! {
			<section
				class={classes!("panel", "builder", self.dragging.is_some().then_some("is-dragging"))}
				aria-labelledby="builder-heading"
			>
				<div class="heading-with-image">
					{ image(images::facility(self.location), "facility-image") }
					<h2 id="builder-heading">{ format!("{} deck builder", self.location.name()) }</h2>
				</div>
				<p class="hint">
					{ "Pick a recipe and fill its deck with your cards, then score it. Card levels and copies come from the cards above." }
				</p>
				<label class="recipe-select">
					<span>{ "Recipe" }</span>
					<select onchange={on_recipe}>
						<option value="" selected={builder.recipe.is_none()}>{ "Choose a recipe…" }</option>
						{ for self.location_recipes().map(|i| html! {
							<option value={i.to_string()} selected={builder.recipe == Some(i)}>
								{ &self.recipes[i].name }
							</option>
						}) }
					</select>
				</label>
				if let Some(recipe) = builder.recipe {
					{ self.view_builder_recipe(recipe) }
					{ self.view_builder_deck(ctx, builder, recipe) }
				}
			</section>
		}
	}

	/// The selected recipe: its image, name, card count, star thresholds and power.
	fn view_builder_recipe(&self, recipe: usize) -> Html {
		let recipe = &self.recipes[recipe];
		html! {
			<div class="recipe-info">
				{ image(images::recipe(recipe), "recipe-info-image") }
				<div class="recipe-info-text">
					<h3>{ &recipe.name }</h3>
					<p class="muted">
						{ format!("{} cards", recipe.cards) }
						{ for recipe.stars.iter().enumerate().map(|(i, threshold)| html! {
							<span class="threshold">
								<span class="stars" aria-label={format!("{} stars at", i + 1)}>{ str::repeat("★", i + 1) }</span>
								{ format!(" {threshold}") }
							</span>
						}) }
					</p>
					if let Some(power) = recipe.power() {
						<p class="power">
							<span class="power-label">{ "Power" }</span>
							{ power }
						</p>
					}
				</div>
			</div>
		}
	}

	/// The available cards, the deck's slots, and scoring.
	fn view_builder_deck(&self, ctx: &Context<Self>, builder: &Builder, recipe: usize) -> Html {
		let link = ctx.link();
		let available = self.available();
		let filled = builder.slots.iter().flatten().count();
		let stats = builder.current_stats(&available);
		let scoring = stats.is_some_and(|stats| stats.result.is_none());
		let inventory_target = self.dragging.is_some_and(|source| matches!(source, DragSource::Slot(_)))
			&& self.drop_target == Some(DropTarget::Inventory);
		let (ondragover, ondragleave, ondrop) = drop_handlers(link, DropTarget::Inventory);

		// How the samples setting would score this deck
		let plan = builder.deck(&available).map(|cards| {
			let orders = deck::distinct_orders(&cards);
			let rolls = deck::rolls_per_order(orders, self.samples);
			(orders, rolls, orders * rolls)
		});
		let status = match plan {
			None => {
				let left = builder.slots.len() - filled;
				format!("Add {left} more {} to score the deck.", if left == 1 { "card" } else { "cards" })
			}
			Some((_, _, crafts)) if scoring => format!("Scoring {} crafts…", group_digits(crafts)),
			Some((orders, rolls, _)) => crate::format::sampling(orders, rolls),
		};

		html! {
			<>
				<h3 class="subheading">{ "Available cards" }</h3>
				<p class="hint">
					{ "Drag a card into a slot, or click it to fill the next empty slot. Drag a deck card back here, or click it, to take it out." }
				</p>
				<div
					class={classes!("builder-inventory", inventory_target.then_some("drop-active"))}
					{ondragover}
					{ondragleave}
					{ondrop}
				>
					if available.is_empty() {
						<p class="muted">{ "No cards are available here. Give some cards a limit above 0 in the cards above." }</p>
					}
					{ for available.iter().map(|&(card, limit)| self.view_builder_card(link, builder, card, limit)) }
				</div>

				<h3 class="subheading">{ format!("Deck: {filled} of {} cards", builder.slots.len()) }</h3>
				<ol class="slots">
					{ for builder.slots.iter().enumerate().map(|(i, &slot)| {
						let card = slot.and_then(|kind| available.iter().find(|(card, _)| card.kind == kind).map(|&(card, _)| card));
						self.view_slot(link, i, card)
					}) }
				</ol>

				<div class="builder-actions">
					<button
						class="button primary"
						disabled={!builder.is_full() || scoring}
						onclick={link.callback(|_| Msg::ScoreDeck)}
					>
						{ if stats.is_some_and(|stats| stats.result.is_some()) { "Score again" } else { "Score deck" } }
					</button>
					<button class="button subtle" disabled={filled == 0} onclick={link.callback(|_| Msg::BuilderClear)}>
						{ "Clear deck" }
					</button>
					{ self.view_samples(link, scoring) }
					<span class="status" aria-live="polite">{ status }</span>
				</div>

				if let Some(result) = stats.and_then(|stats| stats.result.as_ref()) {
					{ self.view_stats(link, recipe, result) }
				}
			</>
		}
	}

	/// One of the location's available cards, with how many copies are left.
	fn view_builder_card(&self, link: &Scope<Self>, builder: &Builder, card: Card, limit: usize) -> Html {
		let kind = card.kind;
		let remaining = limit.saturating_sub(builder.count(kind));
		let usable = remaining > 0;
		let dragging = self.dragging == Some(DragSource::Inventory(kind));
		let title = if usable {
			"Drag into a slot, or click to add"
		} else {
			"Every copy is in the deck"
		};

		html! {
			<div
				class={classes!("card-tile", card_theme(kind), "builder-card", (!usable).then_some("used-up"), dragging.then_some("dragging"))}
				role="button"
				tabindex={if usable { "0" } else { "-1" }}
				aria-disabled={(!usable).to_string()}
				aria-label={format!("{} level {}, {remaining} of {limit} left", kind.name(), card.level)}
				{title}
				draggable={usable.to_string()}
				ondragstart={link.callback(move |e: DragEvent| {
					start_drag(&e);
					Msg::DragStart(DragSource::Inventory(kind))
				})}
				ondragend={link.callback(|_: DragEvent| Msg::DragEnd)}
				onclick={link.callback(move |_| Msg::BuilderAdd(kind))}
				onkeydown={on_activate(link, move || Msg::BuilderAdd(kind))}
			>
				{ card_tile_face(card) }
				<span class="card-tile-badge copies" title={format!("{remaining} of {limit} left")}>
					{ format!("{remaining}/{limit}") }
				</span>
				if !usable {
					<span class="card-tile-check" aria-hidden="true">{ "✓" }</span>
				}
			</div>
		}
	}

	/// A deck slot, empty or holding `card`.
	fn view_slot(&self, link: &Scope<Self>, slot: usize, card: Option<Card>) -> Html {
		let target = DropTarget::Slot(slot);
		let over = self.drop_target == Some(target);
		let dragging = self.dragging == Some(DragSource::Slot(slot));
		let (ondragover, ondragleave, ondrop) = drop_handlers(link, target);

		html! {
			<li
				class={classes!(
					"slot",
					card.is_none().then_some("empty"),
					over.then_some("drop-active"),
					dragging.then_some("dragging"),
				)}
				{ondragover}
				{ondragleave}
				{ondrop}
			>
				if let Some(card) = card {
					<div
						class={classes!("card-tile", card_theme(card.kind), "slot-card")}
						role="button"
						tabindex="0"
						aria-label={format!("Slot {}: {} level {}. Take out", slot + 1, card.kind.name(), card.level)}
						title="Drag to another slot, or click to take out"
						draggable="true"
						ondragstart={link.callback(move |e: DragEvent| {
							start_drag(&e);
							Msg::DragStart(DragSource::Slot(slot))
						})}
						ondragend={link.callback(|_: DragEvent| Msg::DragEnd)}
						onclick={link.callback(move |_| Msg::BuilderRemove(slot))}
						onkeydown={on_activate(link, move || Msg::BuilderRemove(slot))}
					>
						{ card_tile_face(card) }
					</div>
				} else {
					<span class="slot-empty">{ "Empty" }</span>
				}
			</li>
		}
	}

	fn view_stats(&self, link: &Scope<Self>, recipe_index: usize, stats: &DeckStats) -> Html {
		let recipe = &self.recipes[recipe_index];
		let played = |label: &str, played: &crate::deck::Played| {
			html! {
				<div class="stat">
					<span class="stat-label">{ label }</span>
					<span class="stat-value">{ played.total() }</span>
					<span class="stat-detail">
						{ format!("{} × {} ", played.left, played.right) }
						{ view_stars(recipe, played.total()) }
					</span>
				</div>
			}
		};
		// The best deck scored the same way, to show its star chances beside these.
		// Not when this is the best deck: the bars would match.
		let best = self
			.best_stats_for(recipe_index)
			.and_then(|best| best.result.as_ref())
			.filter(|best| best.cards != stats.cards);
		let best_shares = best.map(star_shares);
		html! {
			<div class="stats">
				<h3 class="subheading">{ "Deck stats" }</h3>
				<div class="stat-grid">
					<div class="stat">
						<span class="stat-label">{ "Average" }</span>
						<span class="stat-value">{ stats.average }</span>
						<span class="stat-detail">{ view_stars(recipe, stats.average) }</span>
					</div>
					{ played("Lowest seen", &stats.minimum) }
					{ played("Highest seen", &stats.maximum) }
					<div class="stat">
						<span class="stat-label">{ "Play orders" }</span>
						<span class="stat-value">{ stats.orders }</span>
						<span class="stat-detail">
							{ match stats.rolls {
								1 => format!("scored once each, {} crafts", group_digits(stats.crafts())),
								rolls => format!("scored {rolls} times each, {} crafts", group_digits(stats.crafts())),
							} }
						</span>
					</div>
				</div>

				{ self.view_comparison(link, recipe_index, stats) }

				if !recipe.stars.is_empty() {
					<h4>{ "Chance of each star" }</h4>
					if best.is_some() {
						<p class="star-legend">
							<span class="legend-you">{ "Your deck" }</span>
							<span class="legend-best">{ "Best deck" }</span>
						</p>
					}
					<ul class={classes!("star-chances", best.is_some().then_some("with-best"))}>
						{ for star_shares(stats).into_iter().enumerate().map(|(stars, (share, shown))| {
							let range = star_range(recipe, stars);
							let best_share = best_shares.as_ref().map(|shares| shares[stars].clone());
							html! {
								<li>
									<span class="stars" aria-label={format!("{stars} stars")}>
										{ format!("{}{}", str::repeat("★", stars), str::repeat("☆", recipe.stars.len() - stars)) }
									</span>
									<span class="muted">{ range }</span>
									<span class="meters" aria-hidden="true">
										<span class="meter">
											<span class="meter-fill" style={format!("width: {share:.1}%")}></span>
										</span>
										if let Some((best_share, _)) = &best_share {
											<span class="meter best">
												<span class="meter-fill" style={format!("width: {best_share:.1}%")}></span>
											</span>
										}
									</span>
									<span class="share">
										<span>{ shown }</span>
										if let Some((_, best_shown)) = best_share {
											<span class="best-share" aria-label={format!("best deck {best_shown}")}>{ best_shown }</span>
										}
									</span>
								</li>
							}
						}) }
					</ul>
				}

				<h4>{ "Highest-scoring play order" }</h4>
				{ view_order(&stats.maximum.order) }
				<h4>{ "Lowest-scoring play order" }</h4>
				{ view_order(&stats.minimum.order) }
				<p class="hint">
					{ "Each play order is scored several times to even out random effects. Lowest and highest are the extremes of every craft, so they spread wider with more samples. Raise the samples for steadier results." }
				</p>
			</div>
		}
	}

	/// How a scored deck compares with the best deck for its recipe, or the
	/// search for the best deck while it runs.
	fn view_comparison(&self, link: &Scope<Self>, recipe_index: usize, stats: &DeckStats) -> Html {
		let recipe = &self.recipes[recipe_index];
		let body = match self.known_best(recipe_index) {
			Some(KnownBest { best: Some(best), .. }) => {
				let sampled = self.best_stats_for(recipe_index);
				let best_average = self.best_average(recipe_index).unwrap_or(best.average);
				let difference = stats.average - best_average;
				let tone = match difference.signum() {
					-1 => "behind",
					1 => "ahead",
					_ => "even",
				};
				let share = (best_average > 0).then(|| stats.average as f64 / best_average as f64 * 100.0);
				// Both are sorted
				let is_best = stats.cards == best.cards;
				html! {
					<>
						<div class="stat-grid compare-grid">
							<div class="stat">
								<span class="stat-label">{ "Your average" }</span>
								<span class="stat-value">{ stats.average }</span>
								<span class="stat-detail">{ view_stars(recipe, stats.average) }</span>
							</div>
							<div class="stat">
								<span class="stat-label">{ "Best average" }</span>
								<span class="stat-value">{ best_average }</span>
								<span class="stat-detail">
									{ view_stars(recipe, best_average) }
									{ match sampled {
										Some(BestStats { result: Some(result), .. }) => html! {
											<span class="best-sampling">{ format!("{} crafts", group_digits(result.crafts())) }</span>
										},
										Some(_) => html! {
											<span class="best-sampling">{ "Sampling…" }</span>
										},
										None => html! {
											<button
												class="button subtle"
												title="The best average is from one roll per play order. Score the best deck with your samples setting to compare like for like."
												onclick={link.callback(|_| Msg::ScoreBest)}
											>
												{ "Sample" }
											</button>
										},
									} }
								</span>
							</div>
							<div class={classes!("stat", "difference", tone)}>
								<span class="stat-label">{ "Difference" }</span>
								<span class="stat-value">{ format!("{difference:+}") }</span>
								<span class="stat-detail">
									{ share.map_or(String::new(), |share| format!("{share:.1}% of the best")) }
								</span>
							</div>
						</div>
						if is_best {
							<p class="compare-note">
								{ "This is the best deck for your cards." }
								if difference != 0 {
									{ " The averages differ only by random rolls." }
								}
							</p>
						} else {
							<div class="swaps">
								<div class="swap">
									<span class="swap-label">{ "Best deck" }</span>
									{ view_cards(&best.cards) }
								</div>
							</div>
						}
					</>
				}
			}
			Some(KnownBest { best: None, .. }) => html! {
				<p class="muted">{ "There aren't enough cards to find a best deck for this recipe." }</p>
			},
			None => match self.current_search(recipe_index) {
				Some(search) => {
					let parts = search.run.parts();
					html! {
						<div class="builder-actions">
							<span class="status" aria-live="polite">
								{ format!("Finding the best deck… {} of {parts} parts done", parts - search.jobs_left) }
							</span>
							<button class="button subtle" onclick={link.callback(|_| Msg::StopFindBest)}>{ "Stop" }</button>
						</div>
					}
				}
				None => html! {
					<div class="builder-actions">
						<span class="muted">{ "The best deck for your current cards hasn't been found yet." }</span>
						<button class="button subtle" onclick={link.callback(|_| Msg::FindBest)}>{ "Find best deck" }</button>
					</div>
				},
			},
		};

		html! {
			<div class="comparison">
				<h4>{ "Compared with the best deck" }</h4>
				{ body }
			</div>
		}
	}

	/// The column beside the deck builder summarising every deck scored for its
	/// recipe, newest first.
	fn view_history(&self, link: &Scope<Self>) -> Html {
		let builder = self.builder();
		let Some(recipe_index) = builder.recipe else {
			return html! {
				<aside class="panel history" aria-labelledby="history-heading">
					<h2 id="history-heading">{ "Scoring history" }</h2>
					<p class="hint history-empty">{ "Pick a recipe to see the decks you've scored for it." }</p>
				</aside>
			};
		};
		let recipe = &self.recipes[recipe_index];
		let entries = self.history_for(recipe_index);
		let available = self.available();
		let current = builder.deck(&available);
		let best = self.best_average(recipe_index);
		let top = entries.iter().map(|entry| entry.average).max();
		let top_star = recipe.stars.len();

		html! {
			<aside class="panel history" aria-labelledby="history-heading">
				<div class="history-header">
					<div>
						<h2 id="history-heading">{ "Scoring history" }</h2>
						<p class="hint">
							{ format!(
								"{}: {} {} scored, newest first.",
								recipe.name,
								entries.len(),
								if entries.len() == 1 { "deck" } else { "decks" },
							) }
						</p>
					</div>
					if !entries.is_empty() {
						<button class="button subtle" onclick={link.callback(|_| Msg::ClearHistory)}>{ "Clear" }</button>
					}
				</div>
				if entries.is_empty() {
					<p class="hint history-empty">
						{ "Decks you score for this recipe are listed here, so you can compare them. They're saved in this browser." }
					</p>
				}
				<ol class="history-list">
					{ for entries.iter().enumerate().map(|(i, entry)| {
						let is_current = current.as_ref() == Some(&entry.cards);
						let is_top = top == Some(entry.average);
						// Scored with card levels that have changed since
						let outdated = entry.cards.iter().any(|card| {
							available.iter().find(|(owned, _)| owned.kind == card.kind).map(|(owned, _)| owned.level)
								!= Some(card.level)
						});
						let top_share = entry.star_orders.last().map(|&count| {
							percent(count as f64 / entry.crafts().max(1) as f64 * 100.0)
						});
						html! {
							<li class={classes!("history-entry", is_current.then_some("current"))}>
								<div class="history-score">
									<span class="history-average">{ entry.average }</span>
									{ view_stars(recipe, entry.average) }
									if let Some(best) = best {
										<span
											class={classes!("history-diff", match (entry.average - best).signum() {
												-1 => "behind",
												1 => "ahead",
												_ => "even",
											})}
											title={format!("Compared with the best deck's average of {}", best)}
										>
											{ format!("{:+} vs best", entry.average - best) }
										</span>
									}
								</div>
								<dl class="history-figures">
									<div><dt>{ "Low" }</dt><dd>{ entry.minimum.total() }</dd></div>
									<div><dt>{ "High" }</dt><dd>{ entry.maximum.total() }</dd></div>
									if let (true, Some(share)) = (top_star > 0, top_share) {
										<div><dt>{ format!("{top_star}★") }</dt><dd>{ share }</dd></div>
									}
								</dl>
								{ view_cards(&entry.cards) }
								<div class="history-footer">
									<div class="tags">
										if is_current {
											<span class="tag">{ "In the builder" }</span>
										}
										if is_top {
											<span class="tag good">{ "Highest average" }</span>
										}
										if outdated {
											<span class="tag muted" title="Some card levels have changed since this deck was scored">
												{ "Older card levels" }
											</span>
										}
									</div>
									<div class="history-actions">
										<button class="button subtle" onclick={link.callback(move |_| Msg::LoadDeck(i))}>
											{ "Load" }
										</button>
										<button
											class="button subtle"
											aria-label="Remove from history"
											title="Remove from history"
											onclick={link.callback(move |_| Msg::RemoveHistory(i))}
										>
											{ "✕" }
										</button>
									</div>
								</div>
							</li>
						}
					}) }
				</ol>
			</aside>
		}
	}
}

/// A share of play orders as a percentage, without rounding a few orders down to 0%.
impl App {
	/// The card guide: one section per card at the current location.
	fn view_card_guide(&self) -> Html {
		let location = self.location;
		let facility = self.facility();
		let level_note = match facility.level {
			Some(level) => format!(
				" Rows your {} (level {level}) hasn't reached are faded, and the row for your level is highlighted.",
				location.name()
			),
			None => " Set your facility level on the other pages to highlight the row for your level.".into(),
		};
		html! {
			<>
				<section class="panel" aria-labelledby="guide-heading">
					<div class="heading-with-image">
						{ image(images::facility(location), "facility-image") }
						<h2 id="guide-heading">{ format!("{} cards", location.name()) }</h2>
					</div>
					<p class="hint guide-hint">
						{ "How each card scores, and each facility level where it unlocks, levels up or allows more copies per deck, with its values at that card level. What changed at each row is in bold." }
						{ level_note }
						if location == Location::Alchemy {
							{ " Alchemy colours can't drop below 1." }
						}
						{ " Values marked † aren't confirmed: that level isn't in the data yet, so the default is shown." }
					</p>
				</section>
				<div class="card-guide">
					{ for info::guide_order(&self.facilities, location).into_iter().map(|kind| self.view_card_info(&facility, kind)) }
				</div>
			</>
		}
	}

	fn view_card_info(&self, facility: &Facility, kind: CardKind) -> Html {
		let steps = info::progression(&self.facilities, kind);
		let max_level = info::max_level(&self.facilities, &self.tables, kind);
		let values = info::values(&self.tables, kind, max_level);
		let used = facility.cap(&self.facilities, self.location, kind).apply(facility.owned(kind));
		let yours = (used.limit > 0).then_some(used.level);
		let heading_id = format!("card-{}", images::file_stem(kind.name()));
		let unlock_text = match info::unlock_level(&steps) {
			Some(level) => format!("Unlocks at facility level {level}"),
			None => "Unlock level unknown".into(),
		};

		// One row per facility level where the card unlocks, levels up or gains
		// copies. Without facility data, one row per card level.
		let rows: Vec<(Option<info::Step>, &info::Row)> = if steps.is_empty() {
			values.rows.iter().map(|row| (None, row)).collect()
		} else {
			steps
				.iter()
				.filter(|step| step.card_level > 0)
				.filter_map(|step| Some((Some(*step), values.rows.get(usize::from(step.card_level) - 1)?)))
				.collect()
		};
		// The row in effect at the player's facility level
		let current = facility.level.and_then(|have| {
			rows.iter().rposition(|(step, _)| step.is_some_and(|step| step.facility_level <= have))
		});

		html! {
			<section class="panel card-info" aria-labelledby={heading_id.clone()}>
				<div class={classes!("card-detail", card_theme(kind))}>
					<div class="card-detail-art">{ image(images::card(kind), "card-detail-image") }</div>
					<div class="card-detail-body">
						<h2 class="card-detail-name" id={heading_id}>{ kind.name() }</h2>
						<p class="card-detail-text">{ info::description(kind) }</p>
					</div>
				</div>
				<div class="card-info-main">
					<div class="card-info-meta">
						<span class="card-info-unlock">{ unlock_text }</span>
						if let Some(level) = yours {
							<span class="chip card-info-yours">{ format!("Yours: Lv {level}") }</span>
						}
					</div>
					<div class="table-wrap">
						<table class="card-values">
							<thead>
								<tr>
									<th scope="col">{ "Facility level" }</th>
									<th scope="col">{ "Card level" }</th>
									<th scope="col">{ "Copies" }</th>
									{ for values.columns.iter().map(|column| html! { <th scope="col">{ *column }</th> }) }
								</tr>
							</thead>
							<tbody>
								{ for rows.iter().enumerate().map(|(i, &(step, row))| {
									let previous = i.checked_sub(1).and_then(|prev| rows[prev].0);
									let level_changed = previous.is_none_or(|prev| step.is_none_or(|step| step.card_level != prev.card_level));
									let copies_changed = previous.is_none_or(|prev| step.is_none_or(|step| step.limit != prev.limit));
									let reached = match (facility.level, step) {
										(Some(have), Some(step)) => step.facility_level <= have,
										_ => true,
									};
									let is_current = current == Some(i);
									let dash = || "—".to_string();
									html! {
										<tr
											class={classes!(is_current.then_some("current"), (!reached).then_some("unreached"))}
											aria-current={is_current.then_some("true")}
										>
											<th scope="row">{ step.map_or_else(dash, |step| step.facility_level.to_string()) }</th>
											<td class={classes!(level_changed.then_some("changed"))}>{ format!("Lv {}", row.level) }</td>
											<td class={classes!(copies_changed.then_some("changed"))}>
												{ step.map_or_else(dash, |step| step.limit.to_string()) }
											</td>
											{ for row.cells.iter().map(|cell| html! {
												<td class={classes!((!row.listed).then_some("unconfirmed"))}>
													{ cell }
													if !row.listed {
														<span class="dagger" title="Not in the data yet: shows the default value">{ "†" }</span>
													}
												</td>
											}) }
										</tr>
									}
								}) }
							</tbody>
						</table>
					</div>
				</div>
			</section>
		}
	}
}

impl App {
	/// The facility guide: what unlocks at each level of the current location.
	fn view_facility_guide(&self) -> Html {
		let location = self.location;
		let yours = self.facility().level;
		let levels = info::facility_levels(&self.facilities, location);
		let level_note = match yours {
			Some(level) => format!(" Your {} is level {level}, highlighted below.", location.name()),
			None => " Set your facility level on the other pages to highlight it here.".into(),
		};
		html! {
			<section class="panel" aria-labelledby="facility-guide-heading">
				<div class="heading-with-image">
					{ image(images::facility(location), "facility-image") }
					<h2 id="facility-guide-heading">{ format!("{} levels", location.name()) }</h2>
				</div>
				<p class="hint">
					{ "The recipes and cards each facility level unlocks, and the cards that level up or allow more copies per deck." }
					{ level_note }
				</p>
				<div class="table-wrap">
					<table class="facility-levels">
						<thead>
							<tr>
								<th scope="col">{ "Level" }</th>
								<th scope="col">{ "Recipes" }</th>
								<th scope="col">{ "Cards" }</th>
							</tr>
						</thead>
						<tbody>
							{ for levels.iter().map(|level| {
								let current = yours == Some(level.level);
								html! {
									<tr class={classes!(current.then_some("current"))} aria-current={current.then_some("true")}>
										<th scope="row">{ level.level }</th>
										<td>
											if level.recipes.is_empty() {
												<span class="muted">{ "—" }</span>
											} else {
												<ul class="deck">
													{ for level.recipes.iter().map(|name| self.view_recipe_chip(name)) }
												</ul>
											}
										</td>
										<td>
											if level.cards.is_empty() {
												<span class="muted">{ "—" }</span>
											} else {
												<ul class="deck">
													{ for level.cards.iter().map(view_card_change) }
												</ul>
											}
										</td>
									</tr>
								}
							}) }
						</tbody>
					</table>
				</div>
			</section>
		}
	}

	fn view_recipe_chip(&self, name: &str) -> Html {
		let recipe = self.recipes.iter().find(|recipe| recipe.name == name);
		html! {
			<li class="chip">
				if let Some(recipe) = recipe {
					{ image(images::recipe(recipe), "chip-image") }
				}
				<span>{ name.to_string() }</span>
			</li>
		}
	}
}

/// A card that unlocked, levelled up or gained copies at a facility level.
fn view_card_change(change: &info::CardChange) -> Html {
	let mut details = Vec::new();
	if let Some(level) = change.card_level {
		details.push(format!("Lv {level}"));
	}
	if let Some(limit) = change.limit {
		details.push(if limit == 1 { "1 copy".to_string() } else { format!("{limit} copies") });
	}
	html! {
		<li class={classes!("chip", "card-change", change.new.then_some("new"))}>
			{ image(images::card(change.kind), "chip-image") }
			<span>{ change.kind.name() }</span>
			if change.new {
				<span class="badge">{ "New" }</span>
			}
			<span class="level">{ details.join(" · ") }</span>
		</li>
	}
}

fn percent(share: f64) -> String {
	if share > 0.0 && share < 1.0 {
		"<1%".to_string()
	} else if share < 100.0 && share > 99.0 {
		">99%".to_string()
	} else {
		format!("{share:.0}%")
	}
}

/// The stars `score` earns for `recipe`, filled, then the ones it doesn't, hollow.
fn view_stars(recipe: &Recipe, score: i64) -> Html {
	let earned = recipe.stars(score);
	let total = recipe.stars.len();
	html! {
		<span class="stars" aria-label={format!("{earned} of {total} stars")}>
			{ format!("{}{}", "★".repeat(earned), "☆".repeat(total - earned)) }
		</span>
	}
}

/// Cards in the order they're played.
fn view_order(cards: &[Card]) -> Html {
	html! {
		<ol class="deck play-order">
			{ for cards.iter().map(|card| html! {
				<li class={classes!("card-tile", card_theme(card.kind))}>{ card_tile_face(*card) }</li>
			}) }
		</ol>
	}
}

/// Sets up a drag so every browser starts it.
fn start_drag(e: &DragEvent) {
	if let Some(data) = e.data_transfer() {
		// Firefox won't start a drag without some data
		let _ = data.set_data("text/plain", "");
		data.set_effect_allowed("move");
	}
}

/// The dragover, dragleave and drop handlers for somewhere a card can be dropped.
fn drop_handlers(
	link: &Scope<App>,
	target: DropTarget,
) -> (Callback<DragEvent>, Callback<DragEvent>, Callback<DragEvent>) {
	let over = link.callback(move |e: DragEvent| {
		// Allows dropping here
		e.prevent_default();
		Msg::DragOver(target)
	});
	let leave = link.batch_callback(move |e: DragEvent| {
		// Moving onto a child element also fires dragleave: only leave when the
		// pointer has left this element altogether
		let current = e.current_target()?.unchecked_into::<Node>();
		let related = e.related_target().map(|related| related.unchecked_into::<Node>());
		(!current.contains(related.as_ref())).then_some(Msg::DragLeave(target))
	});
	let drop = link.callback(move |e: DragEvent| {
		// Stops the browser opening the dragged data as a page
		e.prevent_default();
		Msg::DropCard(target)
	});
	(over, leave, drop)
}

/// A keydown handler that acts like a click for Enter and Space, for elements
/// that act as buttons but can't be `<button>`s, because Firefox won't drag those.
fn on_activate(link: &Scope<App>, msg: impl Fn() -> Msg + 'static) -> Callback<KeyboardEvent> {
	link.batch_callback(move |e: KeyboardEvent| {
		let activate = e.key() == "Enter" || e.key() == " ";
		if activate {
			e.prevent_default();
		}
		activate.then(&msg)
	})
}

/// Lists a deck's cards, with repeated cards shown once with a count.
fn view_cards(cards: &[Card]) -> Html {
	let mut groups: Vec<(Card, usize)> = Vec::new();
	for &card in cards {
		match groups.last_mut() {
			Some((last, count)) if *last == card => *count += 1,
			_ => groups.push((card, 1)),
		}
	}
	html! {
		<ul class="deck">
			{ for groups.into_iter().map(|(card, count)| html! {
				<li class={classes!("card-tile", card_theme(card.kind))}>
					{ card_tile_face(card) }
					if count > 1 {
						<span class="card-tile-badge count" aria-label={format!("{count} copies")}>{ format!("×{count}") }</span>
					}
				</li>
			}) }
		</ul>
	}
}

/// A card as the game shows it in a deck: its art, its level in the corner and
/// its name on a banner across the bottom. Goes inside a `.card-tile` element.
fn card_tile_face(card: Card) -> Html {
	html! {
		<>
			<span class="card-tile-art">{ image(images::card(card.kind), "card-tile-image") }</span>
			<span class="card-tile-level">{ format!("Lv {}", card.level) }</span>
			<span class="card-tile-name">{ card.kind.name() }</span>
		</>
	}
}

/// The class that gives a card its facility's colours, like the game's cards.
fn card_theme(kind: CardKind) -> &'static str {
	match kind.location() {
		Location::Kitchen => "card-kitchen",
		Location::Forge => "card-forge",
		Location::Alchemy => "card-alchemy",
	}
}

/// An image from the `static` folder, next to text that already names it, so it
/// has no alt text. It hides itself if the file is missing, and shows again when
/// Yew reuses the element for an image that loads.
fn image(src: String, class: &'static str) -> Html {
	let on_load = Callback::from(|e: Event| e.target_unchecked_into::<HtmlElement>().set_hidden(false));
	let on_error = Callback::from(|e: Event| e.target_unchecked_into::<HtmlElement>().set_hidden(true));
	html! {
		<img class={classes!("image", class)} {src} alt="" loading="lazy" onload={on_load} onerror={on_error} />
	}
}

/// One worker per CPU core.
fn worker_count() -> usize {
	let cores = web_sys::window().map_or(1.0, |window| window.navigator().hardware_concurrency());
	(cores as usize).clamp(1, MAX_WORKERS)
}

fn load_inventory() -> Inventory {
	let Ok(text) = LocalStorage::get::<String>(STORAGE_KEY) else {
		return data::default_inventory();
	};
	if let Ok(inventory) = ron::from_str(&text) {
		return inventory;
	}
	// Saved before facility levels were added: just the cards at each location
	match ron::from_str::<BTreeMap<Location, BTreeMap<CardKind, Owned>>>(&text) {
		Ok(old) => old
			.into_iter()
			.map(|(location, cards)| (location, Facility { level: None, cards }))
			.collect(),
		Err(_) => data::default_inventory(),
	}
}

fn save_inventory(inventory: &Inventory) {
	if let Ok(text) = ron::to_string(inventory) {
		// Not being able to save (e.g. storage disabled) only loses the edit on reload
		let _ = LocalStorage::set(STORAGE_KEY, text);
	}
}

/// The deck builder's history saved in this browser, or an empty one if there's
/// none or it can't be read.
fn load_history() -> BTreeMap<String, Vec<DeckStats>> {
	LocalStorage::get::<String>(HISTORY_KEY)
		.ok()
		.and_then(|text| ron::from_str(&text).ok())
		.unwrap_or_default()
}

/// A deck's chance of each star count as one bar, from 0 stars at the left to
/// every star at the right, in one hue that darkens with each star, with a legend
/// that gives every share as text.
fn view_star_distribution(recipe: &Recipe, stats: &DeckStats) -> Html {
	let shares = star_shares(stats);
	let levels = shares.len().saturating_sub(1).max(1);
	// More stars, more ink: a quarter of the star colour for 0 stars, up to all of it
	let colour = |stars: usize| {
		format!(
			"background: color-mix(in srgb, var(--star) {}%, var(--surface-2))",
			25 + 75 * stars / levels
		)
	};
	let summary = shares
		.iter()
		.enumerate()
		.map(|(stars, (_, shown))| format!("{stars} stars {shown}"))
		.collect::<Vec<_>>()
		.join(", ");

	html! {
		<div class="star-distribution">
			<div class="star-bar" role="img" aria-label={format!("Chance of each star: {summary}")}>
				{ for shares.iter().enumerate().filter(|(_, (share, _))| *share > 0.0).map(|(stars, (share, shown))| html! {
					<span
						class="star-segment"
						style={format!("flex: {share:.3} 1 0; {}", colour(stars))}
						title={format!("{stars}★ ({}): {shown} of crafts", star_range(recipe, stars))}
					></span>
				}) }
			</div>
			<ul class="star-bar-legend" aria-hidden="true">
				{ for shares.iter().enumerate().map(|(stars, (_, shown))| html! {
					<li>
						<span class="swatch" style={colour(stars)}></span>
						{ format!("{stars}★ {shown}") }
					</li>
				}) }
			</ul>
		</div>
	}
}

/// The scores that earn exactly `stars` stars for `recipe`, e.g. "6600–13109".
fn star_range(recipe: &Recipe, stars: usize) -> String {
	match (stars.checked_sub(1).map(|i| recipe.stars[i]), recipe.stars.get(stars)) {
		(None, Some(next)) => format!("under {next}"),
		(Some(threshold), Some(next)) => format!("{threshold}–{}", next - 1),
		(Some(threshold), None) => format!("{threshold}+"),
		(None, None) => String::new(),
	}
}

/// `n` with commas between groups of three digits, e.g. 1,000,000.
fn save_history(history: &BTreeMap<String, Vec<DeckStats>>) {
	if let Ok(text) = ron::to_string(history) {
		// Not being able to save only loses the history on reload
		let _ = LocalStorage::set(HISTORY_KEY, text);
	}
}
