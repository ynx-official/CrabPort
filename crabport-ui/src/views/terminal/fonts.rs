use std::sync::OnceLock;

use gpui::{Font, FontStyle, font};

/// Palette built once and reused for every cell of every frame.
pub(crate) fn palette() -> &'static alacritty_terminal::term::color::Colors {
    static P: OnceLock<alacritty_terminal::term::color::Colors> = OnceLock::new();
    P.get_or_init(alacritty_terminal::term::color::Colors::default)
}

/// Pre-built font variants, cloned cheaply per run.
struct Fonts {
    regular: Font,
    bold: Font,
    italic: Font,
    bold_italic: Font,
}

/// Returns the monospace font family name for the current platform.
///
/// `Menlo` is macOS-only; on Windows it doesn't exist and font-kit's
/// fallback picks a font whose metrics don't match the hardcoded
/// `cell_width`, causing character gaps and clipping. We therefore use a
/// platform-native monospace font so the metrics stay consistent.
fn font_family() -> &'static str {
    if cfg!(target_os = "windows") {
        "Consolas"
    } else if cfg!(target_os = "macos") {
        "Menlo"
    } else {
        "DejaVu Sans Mono"
    }
}

fn fonts() -> &'static Fonts {
    static F: OnceLock<Fonts> = OnceLock::new();
    F.get_or_init(|| {
        let base = font(font_family());
        let mut italic = base.clone();
        italic.style = FontStyle::Italic;
        let mut bold_italic = base.clone().bold();
        bold_italic.style = FontStyle::Italic;
        Fonts {
            regular: base.clone(),
            bold: base.bold(),
            italic,
            bold_italic,
        }
    })
}

pub(crate) fn pick_font(bold: bool, italic: bool) -> Font {
    let f = fonts();
    match (bold, italic) {
        (false, false) => f.regular.clone(),
        (true, false) => f.bold.clone(),
        (false, true) => f.italic.clone(),
        (true, true) => f.bold_italic.clone(),
    }
}
