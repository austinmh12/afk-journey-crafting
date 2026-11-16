//! Scoring for AFK Journey homestead crafting, shared by the command line
//! calculator and the web front end.

pub mod card;
pub mod deck;
pub mod facility;
pub mod inventory;
pub mod recipe;
pub mod score;
pub mod tables;

#[cfg(target_arch = "wasm32")]
pub mod web;
