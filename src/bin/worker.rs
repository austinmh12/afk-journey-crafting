//! The web worker that scores decks for the web front end. See `web::worker`.

#[cfg(target_arch = "wasm32")]
fn main() {
	use gloo_worker::Registrable;
	afkjourney_crafting::web::worker::CraftWorker::registrar().register();
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
	eprintln!("The worker only runs in a browser. Build the web front end with `trunk build`.");
}
