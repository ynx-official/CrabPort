# CrabPort

<p align="center">
  <strong>A modern cross-platform SSH / Telnet client built with Rust + GPUI</strong>
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

CrabPort aims to be a simple and easy-to-use cross-platform SSH / Telnet client, integrating terminal and SFTP file management in one app. It is written in Rust, with a UI built on [GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui) (the GPU-accelerated rendering framework powering the Zed editor).

### Key Features

- **Multi-tab terminal** — SSH / Telnet / Serial / local terminal, multi-session switching
- **SFTP file management** — Visual directory browsing with batch upload & download
- **SSH tunnels** — Local / Remote / Dynamic (SOCKS) port forwarding
- **Proxy connections** — SOCKS5 / HTTP(S) proxy, per-host config
- **Encrypted credential storage** — AES-256-GCM at rest
- **Command history & snippets** — Auto-capture, search, and quick execution
- **Configurable theme colors** — Multiple presets, driven by `config.toml`
- **Cross-platform** — macOS / Linux / Windows, x64 and arm64

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
- [x] Telnet connection type
- [x] Configurable theme colors
- [ ] Settings panel (fonts, custom shortcuts)
- [ ] Terminal session sync (shared across windows)
- [ ] Serial connection type
- [ ] Plugin system

## License

[Apache License 2.0](LICENSE)

Copyright © 2026 ch1ll321
