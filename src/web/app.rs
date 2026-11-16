use std::collections::{BTreeMap, VecDeque};

use gloo_storage::{LocalStorage, Storage};
use gloo_worker::{Spawnable, WorkerBridge};
use web_sys::{HtmlInputElement, HtmlSelectElement};
use yew::prelude::*;

use super::data;
use super::worker::{BestDeck, CraftWorker, Job, JobDone};
use crate::card::{Card, CardKind};
use crate::facility::{self, Facilities};
use crate::inventory::{Facility, Inventory, Owned, available_cards};
use crate::recipe::{Location, Recipe};

/// Where the player's edited inventory is saved in the browser.
const STORAGE_KEY: &str = "afkjourney-crafting.inventory";
/// Path to the worker script `trunk` builds from `src/bin/worker.rs`.
const WORKER_PATH: &str = "worker.js";
const MAX_WORKERS: usize = 32;

pub enum Msg {
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
	JobDone {
		run: u32,
		worker: usize,
		done: JobDone,
	},
}

/// The result for one recipe, filled in as its jobs finish.
struct RecipeResult {
	recipe: usize,
	jobs_left: usize,
	best: Option<BestDeck>,
}

/// The workers and queued jobs of a calculation in progress.
struct Run {
	location: Location,
	workers: Vec<WorkerBridge<CraftWorker>>,
	idle: Vec<usize>,
	queue: VecDeque<Job>,
}

impl Run {
	/// Hands queued jobs to idle workers.
	fn dispatch(&mut self) {
		while let Some(&worker) = self.idle.last() {
			let Some(job) = self.queue.pop_front() else { break };
			self.idle.pop();
			self.workers[worker].send(job);
		}
	}
}

pub struct App {
	recipes: Vec<Recipe>,
	facilities: Facilities,
	inventory: Inventory,
	location: Location,
	selected: Option<usize>,
	/// The latest results at each location.
	results: BTreeMap<Location, Vec<RecipeResult>>,
	run: Option<Run>,
	/// Increases with every run, so results from a cancelled run are ignored.
	run_id: u32,
}

impl Component for App {
	type Message = Msg;
	type Properties = ();

	fn create(_ctx: &Context<Self>) -> Self {
		Self {
			recipes: data::recipes(),
			facilities: data::facilities(),
			inventory: load_inventory(),
			location: Location::Kitchen,
			selected: None,
			results: BTreeMap::new(),
			run: None,
			run_id: 0,
		}
	}

	fn update(&mut self, ctx: &Context<Self>, msg: Msg) -> bool {
		match msg {
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
			}
			Msg::UseMaximums => {
				let location = self.location;
				let facility = self.inventory.entry(location).or_default();
				for kind in CardKind::ALL.into_iter().filter(|kind| kind.location() == location) {
					let maximum = facility.cap(&self.facilities, location, kind).maximum(facility.owned(kind));
					facility.cards.insert(kind, maximum);
				}
				save_inventory(&self.inventory);
			}
			Msg::SetLevel(kind, level) => {
				self.owned_mut(kind).level = level;
				save_inventory(&self.inventory);
			}
			Msg::SetLimit(kind, limit) => {
				self.owned_mut(kind).limit = limit;
				save_inventory(&self.inventory);
			}
			Msg::ResetInventory => {
				self.inventory = data::default_inventory();
				LocalStorage::delete(STORAGE_KEY);
			}
			Msg::SelectRecipe(recipe) => self.selected = recipe,
			Msg::Calculate => self.start_run(ctx),
			Msg::Cancel => {
				self.run = None;
				self.run_id += 1;
			}
			Msg::JobDone { run, worker, done } => {
				if run != self.run_id {
					return false;
				}
				self.job_done(worker, done);
			}
		}
		true
	}

	fn view(&self, ctx: &Context<Self>) -> Html {
		let link = ctx.link();
		html! {
			<>
				<header class="app-header">
					<h1>{ "Homestead Crafting Calculator" }</h1>
					<nav class="tabs" role="tablist">
						{ for Location::ALL.iter().map(|&location| {
							let active = location == self.location;
							html! {
								<button
									role="tab"
									aria-selected={active.to_string()}
									class={classes!("tab", active.then_some("active"))}
									onclick={link.callback(move |_| Msg::SelectLocation(location))}
								>
									{ location.name() }
								</button>
							}
						}) }
					</nav>
				</header>
				<main>
					{ self.view_inventory(ctx) }
					{ self.view_controls(ctx) }
					{ self.view_results() }
				</main>
			</>
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

	/// Indices of the recipes made at the current location.
	/// Indices of the recipes made at the current location that its facility level has unlocked.
	fn location_recipes(&self) -> impl Iterator<Item = usize> + '_ {
		let facility = self.inventory.get(&self.location);
		(0..self.recipes.len()).filter(move |&i| {
			let recipe = &self.recipes[i];
			recipe.location == self.location
				&& facility.is_none_or(|facility| facility.recipe_unlocked(&self.facilities, recipe))
		})
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
		let cards = available_cards(&self.inventory, &self.facilities, self.location);

		self.run_id += 1;
		let run_id = self.run_id;
		let workers = worker_count();
		let bridges = (0..workers)
			.map(|worker| {
				let link = ctx.link().clone();
				CraftWorker::spawner()
					.callback(move |done| link.send_message(Msg::JobDone { run: run_id, worker, done }))
					.spawn(WORKER_PATH)
			})
			.collect();

		let queue = recipes
			.iter()
			.flat_map(|&recipe| {
				let cards = &cards;
				(0..workers).map(move |part| Job {
					recipe,
					cards: cards.clone(),
					part,
					parts: workers,
				})
			})
			.collect();

		self.results.insert(
			self.location,
			recipes
				.iter()
				.map(|&recipe| RecipeResult {
					recipe,
					jobs_left: workers,
					best: None,
				})
				.collect(),
		);

		let mut run = Run {
			location: self.location,
			workers: bridges,
			idle: (0..workers).rev().collect(),
			queue,
		};
		run.dispatch();
		self.run = Some(run);
	}

	fn job_done(&mut self, worker: usize, done: JobDone) {
		let Some(run) = &mut self.run else { return };
		let rows = self.results.entry(run.location).or_default();
		if let Some(row) = rows.iter_mut().find(|row| row.recipe == done.recipe) {
			row.jobs_left = row.jobs_left.saturating_sub(1);
			if let Some(best) = done.best
				&& row.best.as_ref().is_none_or(|current| best.beats(current))
			{
				row.best = Some(best);
			}
		}

		run.idle.push(worker);
		run.dispatch();
		if rows.iter().all(|row| row.jobs_left == 0) {
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
					<h2 id="inventory-heading">{ format!("{} cards", location.name()) }</h2>
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
					<span class="card-name">{ kind.name() }</span>
					if let Some(note) = note {
						<span class="card-note">{ note }</span>
					}
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
		let done = rows.iter().filter(|row| row.jobs_left == 0).count();
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
				<p class="hint">{ "The deck with the highest average score over every play order. Card order is not a play order." }</p>
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
		let cards = if row.jobs_left > 0 {
			let text = if running { "Calculating…" } else { "Cancelled" };
			html! { <span class="muted">{ text }</span> }
		} else {
			match &row.best {
				Some(best) => view_cards(&best.cards),
				None => html! { <span class="muted">{ format!("Not enough cards: needs {}", recipe.cards) }</span> },
			}
		};
		let summary = row.best.as_ref().filter(|_| row.jobs_left == 0).map(|best| {
			let earned = recipe.stars(best.average);
			let stars = format!("{}{}", "★".repeat(earned), "☆".repeat(recipe.stars.len() - earned));
			html! {
				<div class="recipe-summary">
					{ format!("Avg {} ", best.average) }
					<span class="stars" aria-label={format!("{earned} of {} stars", recipe.stars.len())}>{ stars }</span>
				</div>
			}
		});

		html! {
			<tr>
				<th scope="row">
					<div>{ &recipe.name }</div>
					{ for summary }
				</th>
				<td>{ cards }</td>
			</tr>
		}
	}
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
				<li class="chip">
					if count > 1 {
						<span class="count">{ format!("{count}×") }</span>
					}
					<span>{ card.kind.name() }</span>
					<span class="level">{ format!("Lv {}", card.level) }</span>
				</li>
			}) }
		</ul>
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
