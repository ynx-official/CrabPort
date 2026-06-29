use std::collections::HashMap;
use std::rc::Rc;

use gpui::*;
use rust_i18n::t;

use crate::app::{CrabportApp, SidebarItem, Tab, TabKind};
use crate::color::*;
use crate::components::context_menu::ContextMenuController;
use crate::components::dialog::{AlertController, AlertSeverity, AlertState};
use crate::layouts::connection_form::ConnectionFormState;
use crate::layouts::panel::render_panel;
use crate::layouts::tabbar::render_tab_bar;
use crate::layouts::terminal_toolbar::render_terminal_toolbar;
use crate::views;
use crate::views::hosts::{ConnectionHost, HostsView};
use crate::views::panel::sftp::SftpPanel;
use crate::views::terminal::TerminalView;

pub fn render_content(
    selected: SidebarItem,
    handle: &Entity<CrabportApp>,
    tabs: &[Tab],
    active_tab_id: u64,
    terminal_views: &HashMap<u64, Entity<TerminalView>>,
    hosts: &[ConnectionHost],
    form_entity: Option<&ConnectionFormState>,
    sftp_panel: &Entity<SftpPanel>,
    hosts_view: &Entity<HostsView>,
    alert_controller: &Entity<AlertController>,
    context_menu: &Entity<ContextMenuController>,
    window: &mut Window,
    cx: &mut App,
) -> Div {
    let active_tab = tabs.iter().find(|t| t.id == active_tab_id);
    let handle_c = handle.clone();
    let on_close: Rc<dyn Fn(u64, &mut Window, &mut App)> = Rc::new(move |id, _w, cx| {
        handle_c.update(cx, |app, cx| {
            app.close_tab(id, cx);
        });
    });

    let app_handle = handle.clone();
    let on_new = move |w: &mut Window, cx: &mut App| {
        app_handle.update(cx, |app, cx| {
            app.open_connection_form(w, cx);
        });
    };

    let view: AnyElement = match active_tab.map(|t| t.kind) {
        Some(TabKind::Home) => match selected {
            SidebarItem::Sessions => {
                let app_handle = handle.clone();
                let on_connect = move |host_id: i64, _w: &mut Window, cx: &mut App| {
                    app_handle.update(cx, |app, cx| {
                        app.connect_to_host(host_id, cx);
                    });
                };
                let app_handle_edit = handle.clone();
                let on_edit = move |host_id: i64, w: &mut Window, cx: &mut App| {
                    app_handle_edit.update(cx, |app, cx| {
                        app.edit_host(host_id, w, cx);
                    });
                };
                let app_handle_remove = handle.clone();
                let on_remove = move |host_id: i64, _w: &mut Window, cx: &mut App| {
                    app_handle_remove.update(cx, |app, cx| {
                        app.remove_host(host_id, cx);
                    });
                };

                let on_new_rc: Rc<dyn Fn(&mut Window, &mut App)> = Rc::new(on_new);
                let on_connect_rc: Rc<dyn Fn(i64, &mut Window, &mut App)> = Rc::new(on_connect);
                let on_edit_rc: Rc<dyn Fn(i64, &mut Window, &mut App)> = Rc::new(on_edit);
                let on_remove_rc: Rc<dyn Fn(i64, &mut Window, &mut App)> = Rc::new(on_remove);

                hosts_view.update(cx, |view, cx| {
                    view.set_state(
                        hosts.to_vec(),
                        form_entity.cloned(),
                        Some(on_new_rc),
                        Some(on_connect_rc),
                        Some(on_edit_rc),
                        Some(on_remove_rc),
                        context_menu.clone(),
                        alert_controller.clone(),
                        cx,
                    );
                });

                hosts_view.clone().into_any_element()
            }
            SidebarItem::Tunnels => {
                views::tunnels::render_tunnels_view(|_, _| {}).into_any_element()
            }
            SidebarItem::Snippets => views::snippets::render_snippets_view().into_any_element(),
            SidebarItem::History => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_color(rgb(TEXT_MUTED))
                        .child(selected.label().to_string()),
                )
                .into_any_element(),
        },
        Some(TabKind::Terminal) => {
            if let Some(terminal_entity) = active_tab.and_then(|tab| terminal_views.get(&tab.id)) {
                div()
                    .size_full()
                    .pt_2()
                    .pl_2()
                    .key_context("Terminal")
                    .child(terminal_entity.clone())
                    .into_any_element()
            } else {
                div()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(div().text_color(rgb(TEXT_MUTED)).child("Terminal"))
                    .into_any_element()
            }
        }
        None => div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(div().text_color(rgb(TEXT_MUTED)).child("No tab"))
            .into_any_element(),
    };

    let is_terminal = active_tab
        .map(|t| t.kind == TabKind::Terminal)
        .unwrap_or(false);

    // Read monitor status & metrics from the active TerminalView's backend
    let (status, metrics, sftp_progress) = if is_terminal {
        if let Some(terminal_entity) = active_tab.and_then(|tab| terminal_views.get(&tab.id)) {
            terminal_entity.read_with(cx, |view, _cx| {
                let (status, metrics) = if let Some(m) = view.monitor() {
                    (m.status(), m.metrics())
                } else {
                    (
                        crabport_terminal::terminal::RemoteStatus::Local,
                        crabport_terminal::terminal::RemoteMetrics::default(),
                    )
                };
                // Clone the live SFTP progress snapshot so the toolbar can
                // render it without holding the entity lock across the
                // render call. `None` when no transfer is in flight.
                (status, metrics, view.sftp_progress().cloned())
            })
        } else {
            (
                crabport_terminal::terminal::RemoteStatus::Local,
                crabport_terminal::terminal::RemoteMetrics::default(),
                None,
            )
        }
    } else {
        (
            crabport_terminal::terminal::RemoteStatus::Local,
            crabport_terminal::terminal::RemoteMetrics::default(),
            None,
        )
    };

    // Read SFTP state from the active TerminalView's backend and push it
    // into the shared SftpPanel entity.
    let (sftp_entries, sftp_cwd): (
        std::sync::Arc<Vec<(String, bool)>>,
        Option<std::sync::Arc<String>>,
    ) = if is_terminal {
        if let Some(terminal_entity) = active_tab.and_then(|tab| terminal_views.get(&tab.id)) {
            terminal_entity.read_with(cx, |view, _cx| {
                if view.allow_sftp() {
                    (view.sftp_entries().unwrap_or_default(), view.sftp_cwd())
                } else {
                    (std::sync::Arc::new(Vec::new()), None)
                }
            })
        } else {
            (std::sync::Arc::new(Vec::new()), None)
        }
    } else {
        (std::sync::Arc::new(Vec::new()), None)
    };

    // Build SFTP navigate callback
    let sftp_navigate: Option<Rc<dyn Fn(String, &mut App)>> = if is_terminal {
        active_tab.and_then(|tab| {
            terminal_views.get(&tab.id).map(|entity| {
                let entity = entity.clone();
                Rc::new(move |path: String, cx: &mut App| {
                    entity.read_with(cx, |view, _cx| {
                        view.sftp_navigate(&path);
                    });
                }) as Rc<dyn Fn(String, &mut App)>
            })
        })
    } else {
        None
    };

    // Build SFTP download callback. Mirrors `sftp_navigate`'s shape: a thin
    // closure that forwards `(remote_path, local_path)` to the active
    // terminal's backend. The backend reports completion asynchronously via
    // `BackendEvent::SftpTransferFinished`, which `TerminalView` already
    // surfaces as a status line — no extra plumbing needed here.
    let sftp_download: Option<Rc<dyn Fn(String, String, &mut App)>> = if is_terminal {
        active_tab.and_then(|tab| {
            terminal_views.get(&tab.id).map(|entity| {
                let entity = entity.clone();
                Rc::new(
                    move |remote_path: String, local_path: String, cx: &mut App| {
                        entity.read_with(cx, |view, _cx| {
                            view.sftp_download(&remote_path, &local_path);
                        });
                    },
                ) as Rc<dyn Fn(String, String, &mut App)>
            })
        })
    } else {
        None
    };

    // Build SFTP upload callback. Same shape as `sftp_download` but with the
    // argument order swapped to match `view.sftp_upload(local, remote)`.
    let sftp_upload: Option<Rc<dyn Fn(String, String, &mut App)>> = if is_terminal {
        active_tab.and_then(|tab| {
            terminal_views.get(&tab.id).map(|entity| {
                let entity = entity.clone();
                Rc::new(
                    move |local_path: String, remote_path: String, cx: &mut App| {
                        entity.read_with(cx, |view, _cx| {
                            view.sftp_upload(&local_path, &remote_path);
                        });
                    },
                ) as Rc<dyn Fn(String, String, &mut App)>
            })
        })
    } else {
        None
    };

    // Build SFTP delete callback. Forwards the remote path to the backend's
    // `sftp_delete`, which stats the path to choose `remove_file` vs
    // recursive `remove_dir`.
    let sftp_delete: Option<Rc<dyn Fn(String, &mut App)>> = if is_terminal {
        active_tab.and_then(|tab| {
            terminal_views.get(&tab.id).map(|entity| {
                let entity = entity.clone();
                Rc::new(move |remote_path: String, cx: &mut App| {
                    entity.read_with(cx, |view, _cx| {
                        view.sftp_delete(&remote_path);
                    });
                }) as Rc<dyn Fn(String, &mut App)>
            })
        })
    } else {
        None
    };

    let has_sftp = !sftp_entries.is_empty();
    sftp_panel.update(cx, |panel, cx| {
        panel.set_state(
            sftp_entries,
            sftp_cwd,
            sftp_navigate,
            sftp_download,
            sftp_upload,
            sftp_delete,
            active_tab_id,
            context_menu.clone(),
            alert_controller.clone(),
            window,
            cx,
        );
    });

    // ---- Host-key prompt ----
    //
    // If the active terminal view has a pending host-key prompt (pushed by
    // the SSH backend's `check_server_key` via the verifier), surface it via
    // the global `AlertController`. We only trigger when the controller is
    // idle so we don't re-spawn the dialog on every render while it's
    // already showing — the overlay retains the `PendingHostKey` until the
    // user resolves it (the alert's confirm/cancel callbacks call
    // `TerminalView::resolve_pending_host_key`).
    if is_terminal {
        if let Some(terminal_entity) = active_tab.and_then(|tab| terminal_views.get(&tab.id)) {
            let pending = terminal_entity.read_with(cx, |view, _| view.pending_host_key_info());
            if let Some(info) = pending {
                let controller_busy = alert_controller.read_with(cx, |c, _| c.is_active());
                if !controller_busy {
                    let term_for_confirm = terminal_entity.clone();
                    let on_confirm = Rc::new(move |_w: &mut Window, cx: &mut App| {
                        term_for_confirm.update(cx, |view, _cx| {
                            view.resolve_pending_host_key(true);
                        });
                    });
                    let term_for_cancel = terminal_entity.clone();
                    let on_cancel = Rc::new(move |_w: &mut Window, cx: &mut App| {
                        term_for_cancel.update(cx, |view, _cx| {
                            view.resolve_pending_host_key(false);
                        });
                    });
                    alert_controller.update(cx, |c, cx| {
                        c.show(
                            AlertState {
                                severity: AlertSeverity::Warning,
                                title: t!("terminal.host_key_unknown").to_string().into(),
                                description: {
                                    let host_port = if info.port == 22 {
                                        info.host.clone()
                                    } else {
                                        format!("{}:{}", info.host, info.port)
                                    };
                                    Some(
                                        t!("terminal.host_key_prompt", host = host_port.as_str())
                                            .to_string()
                                            .into(),
                                    )
                                },
                                details: vec![
                                    (
                                        t!("terminal.host_key_algo").to_string().into(),
                                        info.algo.clone().into(),
                                    ),
                                    (
                                        t!("terminal.host_key_fingerprint").to_string().into(),
                                        info.fingerprint.clone().into(),
                                    ),
                                ],
                                confirm_label: t!("terminal.host_key_accept").to_string().into(),
                                cancel_label: t!("terminal.host_key_cancel").to_string().into(),
                                open: true,
                                on_confirm: Some(on_confirm),
                                on_cancel: Some(on_cancel),
                            },
                            cx,
                        );
                    });
                }
            }
        }
    }

    div()
        .flex_1()
        .h_full()
        .bg(rgb(BG_BASE))
        .flex()
        .flex_col()
        .child(render_tab_bar(
            handle,
            tabs,
            active_tab_id,
            active_tab.map(|t| t.kind == TabKind::Home).unwrap_or(false),
            on_close,
        ))
        .child(
            div()
                .flex_1()
                .flex()
                .flex_row()
                .overflow_hidden()
                .child(view)
                .child(render_panel(is_terminal, 0, has_sftp, sftp_panel.clone())),
        )
        .child(render_terminal_toolbar(
            is_terminal,
            status,
            metrics,
            sftp_progress,
        ))
}
