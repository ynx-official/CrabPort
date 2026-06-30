//! Application-wide (macOS) menu bar.
//!
//! The menu bar is set once at app startup via `cx.set_menus(app_menus())`.
//! Each item invokes an action; the actions are either:
//!
//! - Custom actions declared here (`Quit`, `Hide`) handled by global
//!   `cx.on_action` listeners registered in `main` (which call `App::quit` /
//!   `App::hide`).
//! - The `OpenSettings` / `OpenAbout` actions declared here, which open the
//!   corresponding secondary windows.
//! - GPUI built-in edit actions (`Cut` / `Copy` / `Paste` / `Undo` / `Redo` /
//!   `SelectAll`) wired through `MenuItem::os_action`. On macOS these are
//!   dispatched by the system's standard text-edit machinery to the focused
//!   input (e.g. an `InputState`-backed text field), and macOS renders the
//!   standard shortcuts next to them automatically.
//!
//! i18n: labels are looked up via `rust_i18n::t!` so the menu bar matches the
//! active locale. `set_locale` is called from `main` before `app_menus()`.

use gpui::*;
use gpui_component::input::{Copy, Cut, Paste, Redo, SelectAll, Undo};
use rust_i18n::t;

// App-specific menu actions. These have no built-in GPUI/gpui-component
// equivalent, so we declare them here with `actions!`. `main` registers
// global handlers and key bindings for them.
//
// The standard edit actions (Cut/Copy/Paste/Undo/Redo/SelectAll) are NOT
// redefined here — they are imported from `gpui_component::input` above so
// that keyboard shortcuts dispatch to the same action types that
// `InputState`-backed text fields listen for. Re-declaring them with
// `actions!` would create distinct types and break that wiring.
actions!(menus, [Quit, Hide, OpenSettings, OpenAbout, Minimize, Zoom]);

/// Build the macOS application menu bar.
///
/// Layout follows the standard macOS convention:
///
/// ```text
/// CrabPort
///   About CrabPort
///   Settings…
///   ─────────
///   Services ▸            (system-managed submenu)
///   ─────────
///   Hide CrabPort
///   ─────────
///   Quit CrabPort
/// Edit
///   Undo / Redo
///   ─────────
///   Cut / Copy / Paste / Select All
/// Window
///   Minimize  /  Zoom
/// ```
pub fn app_menus() -> Vec<Menu> {
    let app_name: SharedString = t!("menu.app_name").into();

    vec![
        Menu {
            name: app_name.clone(),
            items: vec![
                MenuItem::action(t!("menu.about"), OpenAbout),
                MenuItem::action(t!("menu.settings"), OpenSettings),
                MenuItem::separator(),
                // macOS inserts standard Hide/Hide-Others/Show-All around the
                // Services submenu; using the system submenu keeps that order.
                MenuItem::os_submenu(t!("menu.services"), SystemMenuType::Services),
                MenuItem::separator(),
                MenuItem::action(t!("menu.hide"), Hide),
                MenuItem::separator(),
                MenuItem::action(t!("menu.quit"), Quit),
            ],
        },
        Menu {
            name: t!("menu.edit").into(),
            items: vec![
                MenuItem::os_action(t!("menu.undo"), Undo, OsAction::Undo),
                MenuItem::os_action(t!("menu.redo"), Redo, OsAction::Redo),
                MenuItem::separator(),
                MenuItem::os_action(t!("menu.cut"), Cut, OsAction::Cut),
                MenuItem::os_action(t!("menu.copy"), Copy, OsAction::Copy),
                MenuItem::os_action(t!("menu.paste"), Paste, OsAction::Paste),
                MenuItem::os_action(t!("menu.select_all"), SelectAll, OsAction::SelectAll),
            ],
        },
        Menu {
            name: t!("menu.window").into(),
            items: vec![
                MenuItem::action(t!("menu.minimize"), Minimize),
                MenuItem::action(t!("menu.zoom"), Zoom),
            ],
        },
    ]
}
