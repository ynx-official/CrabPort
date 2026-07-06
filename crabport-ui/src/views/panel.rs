pub mod history_command_panel;
pub mod sftp;
pub mod snippets_panel;
pub mod tunnels_panel;

/// Semantic identity of a right-hand panel pane. Stored on the app as
/// `panel_active_tab` so the user's last selection survives switches
/// between terminal backends whose pane sets differ (e.g. SSH shows all
/// four; Telnet shows only History + Snippets). The positional index used
/// by `Tabs` is derived from this at render time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PanelKind {
    #[default]
    History,
    Snippets,
    Sftp,
    Tunnels,
}
