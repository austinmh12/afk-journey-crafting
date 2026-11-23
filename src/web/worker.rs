//! Scores decks in a web worker so the page stays responsive.
//!
//! The page runs one worker per CPU core and splits each recipe into one
//! [`Job`] per worker: job `part` of `parts` scores every `parts`-th deck,
//! starting at deck `part`. The page then keeps the best of each job's results.
//!
//! The deck builder also asks a worker for the stats of one deck with
//! [`Request::Stats`].

use gloo_worker::{HandlerId, Worker, WorkerScope};
use serde::{Deserialize, Serialize};

use crate::card::Card;
use crate::deck::{Deck, DeckStats, beats, generate_decks};
use crate::recipe::Recipe;
use crate::tables::CardTables;

/// Score a share of one recipe's decks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
	/// Index into [`super::data::recipes`].
	pub recipe: usize,
	/// The cards that may be used, with how many copies of each.
	pub cards: Vec<(Card, usize)>,
	pub part: usize,
	pub parts: usize,
}

/// The best deck a job found.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BestDeck {
	/// Position in [`generate_decks`]'s output, used to break ties.
	pub index: usize,
	pub cards: Vec<Card>,
	/// Average score over every play order.
	pub average: i64,
}

impl BestDeck {
	/// Whether this deck should replace `other` as the best.
	pub fn beats(&self, other: &Self) -> bool {
		beats(self.average, self.index, other.average, other.index)
	}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobDone {
	pub recipe: usize,
	/// `None` when the job had no decks to score, such as when there
	/// aren't enough cards for the recipe.
	pub best: Option<BestDeck>,
}

/// Score every play order of one deck for the deck builder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsJob {
	/// Index into [`super::data::recipes`].
	pub recipe: usize,
	pub cards: Vec<Card>,
	/// About how many crafts to score in all. See [`Deck::stats`].
	pub samples: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsDone {
	pub recipe: usize,
	pub stats: DeckStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Request {
	Best(Job),
	Stats(StatsJob),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Response {
	Best(JobDone),
	Stats(StatsDone),
}

pub struct CraftWorker {
	recipes: Vec<Recipe>,
	tables: CardTables,
}

impl Worker for CraftWorker {
	type Message = ();
	type Input = Request;
	type Output = Response;

	fn create(_scope: &WorkerScope<Self>) -> Self {
		Self {
			recipes: super::data::recipes(),
			tables: super::data::tables(),
		}
	}

	fn update(&mut self, _scope: &WorkerScope<Self>, _msg: Self::Message) {}

	fn received(&mut self, scope: &WorkerScope<Self>, request: Request, id: HandlerId) {
		let response = match request {
			Request::Best(job) => Response::Best(self.best(job)),
			Request::Stats(job) => Response::Stats(self.stats(job)),
		};
		scope.respond(id, response);
	}
}

impl CraftWorker {
	fn best(&self, job: Job) -> JobDone {
		let recipe = &self.recipes[job.recipe];
		let mut rng = rand::rng();
		let mut best: Option<BestDeck> = None;

		let decks = generate_decks(&job.cards, recipe.cards);
		for (index, deck) in decks.into_iter().enumerate().skip(job.part).step_by(job.parts.max(1)) {
			let average = deck.score(recipe, &self.tables, &mut rng).average();
			if best.as_ref().is_none_or(|b| beats(average, index, b.average, b.index)) {
				best = Some(BestDeck { index, cards: deck.cards, average });
			}
		}

		JobDone { recipe: job.recipe, best }
	}

	fn stats(&self, job: StatsJob) -> StatsDone {
		let deck = Deck { cards: job.cards };
		let stats = deck.stats(&self.recipes[job.recipe], &self.tables, job.samples, &mut rand::rng());
		StatsDone { recipe: job.recipe, stats }
	}
}
