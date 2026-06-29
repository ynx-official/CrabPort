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

fn fonts() -> &'static Fonts {
    static F: OnceLock<Fonts> = OnceLock::new();
    F.get_or_init(|| {
        let base = font("Menlo");
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
