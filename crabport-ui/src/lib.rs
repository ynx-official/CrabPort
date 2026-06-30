pub mod app;
pub mod app_state;
pub mod assets;
pub mod color;
pub mod components;
pub mod layouts;
pub mod menus;
pub mod theme;
pub mod views;
pub mod windows;

pub use app::CrabportApp;

rust_i18n::i18n!("i18n", fallback = "en");

/// Set the active locale for translations. Wraps `rust_i18n::set_locale` so
/// the binary crate doesn't need to depend on `rust_i18n` directly.
pub fn set_locale(locale: &str) {
    rust_i18n::set_locale(locale);
}
