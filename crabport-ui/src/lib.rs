pub mod app;
pub mod assets;
pub mod color;
pub mod components;
pub mod layouts;
pub mod theme;
pub mod views;

pub use app::CrabportApp;

rust_i18n::i18n!("i18n", fallback = "en");
