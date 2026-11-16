//! Natively this is the command line calculator. Built for the browser with
//! `trunk`, it's the web front end.

#[cfg(not(target_arch = "wasm32"))]
mod cli;

#[cfg(not(target_arch = "wasm32"))]
fn main() -> std::process::ExitCode {
	cli::main()
}

#[cfg(target_arch = "wasm32")]
fn main() {
	yew::Renderer::<afkjourney_crafting::web::App>::new().render();
}
