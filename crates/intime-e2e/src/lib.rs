//! Scenario replay helpers for staged end-to-end tests.
//!
//! Screenshots come from fixture files on disk (recorded / authored assets).
//! Embeddings go through the real HTTP `EmbeddingService` against a staged
//! server (Docker embed-stub or full CLIP). Persistence uses real SQLite +
//! sqlite-vec via `TestDatabase`.

pub mod capture;
pub mod runner;
pub mod scenario;

pub use capture::FixtureCapture;
pub use runner::{ReplayReport, replay_scenario};
pub use scenario::Scenario;
