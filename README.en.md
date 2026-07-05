# CrabPort

<p align="center">
  <strong>A modern cross-platform SSH / SFTP client built with Rust + GPUI</strong>
</p>

<p align="center">
  <a href="README.md">中文</a> · <a href="README.en.md">English</a>
</p>

<p align="center">
  <a href="https://github.com/chi11321/CrabPort/actions/workflows/dev.yml"><img alt="CI" src="https://github.com/chi11321/CrabPort/actions/workflows/dev.yml/badge.svg?branch=dev"></a>
  <a href="LICENSE"><img alt="License" src="https://img.shields.io/badge/license-Apache--2.0-blue.svg"></a>
  <img alt="Platform" src="https://img.shields.io/badge/platform-macOS%20%7C%20Linux%20%7C%20Windows-lightgrey.svg">
  <img alt="Rust" src="https://img.shields.io/badge/rust-1.91%2B-orange.svg">
</p>

---

## Overview

CrabPort aims to be a simple and easy-to-use cross-platform SSH client, integrating terminal and SFTP file management in one app. It is written in Rust, with a UI built on [GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui) (the GPU-accelerated rendering framework powering the Zed editor).

### Key Features

- **Multi-tab terminal** — Built on `russh` + `alacritty_terminal`, with SSH / Telnet / Serial / local-terminal support, multi-session, tab switching, and full ANSI color rendering
- **SFTP file management** — Visual directory browsing, file/directory upload & download, multi-select batch operations
- **SSH port forwarding / tunnels** — Local (`-L`), Remote (`-R`), and Dynamic (`-D` SOCKS) tunnels; create, start/stop, edit, and delete from the UI
- **Proxy connections** — SOCKS5, HTTP CONNECT, and HTTPS CONNECT proxies; per-host proxy config with optional username/password auth
- **Secure credential storage** — Keys and passwords encrypted with AES-256-GCM; the encryption key is a locally generated random file
- **Command history** — Automatically captures terminal command history with search, save-as-snippet, and one-click paste/run
- **Snippet management** — Globally saved common commands with real-time search and quick execution
- **Host management** — Persistent connection profiles with favorites and sorting by last login
- **SSH host-key verification** — Prompts for confirmation on first connect, then auto-verifies on subsequent connects
- **Settings panel** — Appearance & language (Chinese / English) controls, persisted live to `config.toml`
- **Terminal key bindings** — Platform-native shortcuts (copy/paste) + Ctrl/Shift/Alt modifiers + function keys / arrow keys / Home/End with full VT escape sequences
- **Cross-platform** — Native support for macOS / Linux / Windows on both x64 and arm64 architectures

## Screenshots

![Main UI](imgs/PixPin_2026-07-01_01-19-29.png)

![Terminal & file panel](imgs/PixPin_2026-07-01_01-20-00.png)

## Download & Install

### Pre-built Binaries from Releases

Download the latest version for your platform from the [Releases page](https://github.com/chi11321/CrabPort/releases):

| Platform | Download | Notes |
|----------|----------|-------|
| macOS (Apple Silicon) | `CrabPort-v*-macos-aarch64.dmg` | Open the `.dmg` and drag CrabPort to `/Applications` |
| macOS (Intel) | `CrabPort-v*-macos-x86_64.dmg` | Open the `.dmg` and drag CrabPort to `/Applications` |
| Linux (x64) | `CrabPort-v*-linux-x86_64.AppImage` | `chmod +x` and double-click to run; runtime libs are bundled |
| Linux (arm64) | `CrabPort-v*-linux-aarch64.AppImage` | `chmod +x` and double-click to run; runtime libs are bundled |
| Windows (x64) | `CrabPort-v*-windows-x86_64.zip` | Extract and run `CrabPort.exe` |
| Windows (arm64) | `CrabPort-v*-windows-aarch64.zip` | Extract and run `CrabPort.exe` |

> macOS builds ship as `.dmg` disk images. Linux builds ship as `.AppImage` (bundles the X11 / Wayland / Vulkan / fontconfig runtime libraries, so no manual system-package install is needed). Windows builds ship as a `.zip` (cargo-bundle v0.11.0 has an MSI packaging bug, so no `.msi` installer is provided for now).

**macOS note**: On first launch you may see "cannot verify developer". Right-click the app → select "Open" to bypass, or run in Terminal:
```bash
xattr -cr /Applications/CrabPort.app
```

### Build from Source

#### Prerequisites

- **Rust 1.91+** (recommend installing via [rustup](https://rustup.rs/))
- Platform-native build toolchain

#### Platform-specific Dependencies

**macOS**: Xcode Command Line Tools
```bash
xcode-select --install
```

**Linux** (Debian/Ubuntu):
```bash
sudo apt-get install -y \
  libx11-dev libx11-xcb-dev libxcb1-dev libxcb-randr0-dev \
  libxcb-keysyms1-dev libxcb-icccm4-dev libxcb-image0-dev \
  libxcb-shape0-dev libxcb-xfixes0-dev libxcb-cursor-dev \
  libxkbcommon-dev libxkbcommon-x11-dev \
  libwayland-dev wayland-protocols \
  libgl1-mesa-dev libegl1-mesa-dev libvulkan-dev \
  libfontconfig1-dev libfreetype6-dev \
  libasound2-dev libpulse-dev libdbus-1-dev \
  libssl-dev pkg-config \
  squashfs-tools   # mksquashfs, required for .AppImage bundling
```

**Windows**: MSVC toolchain (ships with Visual Studio Build Tools)

#### Build & Run

```bash
# Clone the repo
git clone https://github.com/chi11321/CrabPort.git
cd CrabPort

# Debug run
cargo run

# Release build
cargo build --release
```

#### Bundle platform installers

Install [cargo-bundle](https://github.com/burtonageo/cargo-bundle) first:

```bash
cargo install cargo-bundle --locked
```

| Platform | Command | Output |
|----------|---------|--------|
| macOS | `cargo bundle --release --format dmg` | `target/release/bundle/dmg/CrabPort_*.dmg` |
| Linux | `cargo bundle --release --format appimage` | `target/release/bundle/appimage/CrabPort_*.AppImage` |
| Windows | `cargo build --release` then zip the `.exe` manually | `CrabPort.exe` (`.zip`) |

> Windows does not use cargo-bundle: its v0.11.0 MSI bundler has a bug that inserts a string into a binary column, so both CI and local builds ship a zipped `.exe` instead.

## Project Structure

CrabPort is organized as a Cargo workspace with cleanly separated crates:

```
CrabPort/
├── src/                    # Binary entry point
│   └── main.rs             # Bootstraps the GPUI Application
├── crabport-core/          # Core infrastructure
│   ├── credential.rs       # Host / credential / proxy / tunnel data models
│   ├── config.rs           # config.toml read/write (LazyLock global)
│   ├── crypto.rs           # AES-256-GCM encrypt/decrypt
│   ├── keybind.rs          # Terminal key bindings & resolution
│   ├── store.rs            # SQLite persistence layer
│   ├── profile.rs          # User config directory
│   └── log.rs              # Logging initialization
├── crabport-ssh/           # SSH backend
│   ├── backend.rs          # russh session management
│   ├── handler.rs          # Connection callbacks & host-key verification
│   ├── keys.rs             # Private-key parsing (OpenSSH/PEM)
│   ├── known_hosts.rs      # known_hosts persistence
│   ├── monitor.rs          # PTY data bridging
│   ├── crabport_tunnel.rs  # TunnelManager / tunnel lifecycle & forward table
│   ├── owned_session.rs    # Session handle wrapper
│   ├── session.rs          # Session connect flow (with proxy dialing)
│   └── transfer/           # SFTP transfer dispatch
├── crabport-sftp/          # SFTP backend
│   ├── api.rs              # SFTP operation trait
│   ├── backend.rs          # russh-sftp implementation
│   ├── archive.rs          # Directory pack/unpack (tar+gz)
│   └── transfer.rs         # Chunked transfer
├── crabport-terminal/      # Terminal abstraction
│   └── terminal.rs         # alacritty_terminal wrapper
├── crabport-proxy/         # Proxy dialing (SOCKS5 / HTTP CONNECT / HTTPS CONNECT)
│   └── lib.rs              # Returns a unified AsyncRead+AsyncWrite stream
├── crabport-tunnel/        # Tunnel data types & reverse-forward registry
│   └── lib.rs              # TunnelInfo / ReverseForwardRegistry, etc.
└── crabport-ui/            # GPUI interface layer
    ├── src/
    │   ├── app.rs          # Main window & tab management
    │   ├── views/
    │   │   ├── terminal/   # Terminal view (render, selection, fonts, colors)
    │   │   ├── panel/      # Right-hand panel (SFTP/History/Snippets)
    │   │   ├── hosts/      # Host list & connection form (with proxy config)
    │   │   ├── tunnels.rs  # Tunnel management view (Local/Remote/Dynamic)
    │   │   └── snippets.rs # Snippet management
    │   ├── windows/        # Settings panel, About, and other aux windows
    │   ├── layouts/        # Layout components (sidebar, command palette, connection form)
    │   └── components/     # Reusable UI components
    ├── assets/             # Icons and static assets
    └── i18n/               # Translations (zh-CN / en)
```

## Data Storage Locations

App data lives under the platform-standard directory:

| Platform | Path |
|----------|------|
| macOS | `~/Library/Application Support/crabport/` |
| Linux | `~/.local/share/crabport/` |
| Windows | `%APPDATA%\crabport\` |

Contains:
- `crabport.db` — SQLite database (hosts, credentials, snippets, tunnels, proxies)
- `.key` — AES-256 encryption key (randomly generated; do not delete — stored credentials cannot be decrypted without it)
- `config.toml` — app configuration (language & appearance settings; written atomically)

## Tech Stack

| Area | Technology |
|------|------------|
| Language | Rust 2024 Edition |
| UI framework | [GPUI](https://github.com/zed-industries/zed) |
| Component library | [gpui-component](https://github.com/longbridge/gpui-component) |
| Animation | [gpui-animation](https://github.com/chi11321/gpui-animation) |
| SSH protocol | [russh](https://github.com/Eugeny/russh) |
| SFTP protocol | [russh-sftp](https://github.com/AspectUnk/russh-sftp) |
| Terminal emulator | [alacritty_terminal](https://github.com/alacritty/alacritty) |
| Database | [rusqlite](https://github.com/rusqlite/rusqlite) (SQLite) |
| Crypto | [aes-gcm](https://github.com/RustCrypto/AEADs) (AES-256-GCM) |
| Async runtime | [tokio](https://tokio.rs) + [smol](https://github.com/smol-rs/smol) |
| i18n | [rust-i18n](https://github.com/longbridge/rust-i18n) |

## Roadmap

- [x] Settings panel (language)
- [x] Port forwarding / SSH tunnel management (Local / Remote / Dynamic)
- [x] Proxy connections (SOCKS5 / HTTP CONNECT / HTTPS CONNECT)
- [x] Telnet / Serial connection types
- [ ] Settings panel (theme, fonts, custom shortcuts)
- [ ] Terminal session sync (shared across windows)
- [ ] Custom color schemes
- [ ] Plugin system

## License

[Apache License 2.0](LICENSE)

Copyright © 2026 ch1ll321
