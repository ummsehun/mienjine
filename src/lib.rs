// 기존 모듈 (Strangler Fig - 유지)
pub mod assets;
pub mod engine;
pub mod render;
pub mod runtime;
pub(crate) mod shared;

// DDD 신규 계층 (Phase 1)
pub mod application;
pub mod domain;
pub mod infrastructure;

// Interfaces 계층 (Phase 5)
pub mod interfaces;

// 기존 re-exports (유지)
pub use assets::loader;
pub use engine::{animation, math, pipeline, scene};
pub use render::renderer;
pub use runtime::{app, cli};
