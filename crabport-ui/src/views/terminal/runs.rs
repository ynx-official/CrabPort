use alacritty_terminal::term::cell::Flags;
use gpui::{TextRun, UnderlineStyle, px, rgb};

use crate::views::terminal::color::*;
use crate::views::terminal::fonts::pick_font;
use crate::views::terminal::render_cache::CellSnap;

pub(crate) fn make_run(
    len: usize,
    bold: bool,
    italic: bool,
    fg: u32,
    inverse: bool,
    inverse_bg: u32,
    underline: bool,
) -> TextRun {
    let run_font = pick_font(bold, italic);
    let fg_color = if inverse { rgb(inverse_bg) } else { rgb(fg) };
    TextRun {
        len,
        font: run_font,
        color: fg_color.into(),
        background_color: None,
        underline: if underline {
            Some(UnderlineStyle {
                color: Some(fg_color.into()),
                thickness: px(1.0),
                wavy: false,
            })
        } else {
            None
        },
        strikethrough: None,
    }
}

pub(crate) fn build_runs(cells: &[CellSnap], num_cols: usize) -> (String, Vec<TextRun>) {
    let mut line_text = String::new();
    let mut runs: Vec<TextRun> = Vec::new();
    let mut run_start = 0usize;
    let mut cur_fg = TERM_FG;
    let mut cur_inv_bg = TERM_BG;
    let mut cur_bold = false;
    let mut cur_italic = false;
    let mut cur_underline = false;
    let mut cur_inverse = false;

    for (ci, cell) in cells.iter().enumerate() {
        if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
            continue;
        }
        let ef = cell.fg;
        let eb = cell.bg;
        let is_b = cell.flags.contains(Flags::BOLD);
        let is_i = cell.flags.contains(Flags::ITALIC);
        let is_u = cell.flags.contains(Flags::UNDERLINE);
        let is_inv = cell.flags.contains(Flags::INVERSE);

        let new_run = ef != cur_fg
            || eb != cur_inv_bg
            || is_b != cur_bold
            || is_i != cur_italic
            || is_u != cur_underline
            || is_inv != cur_inverse;

        if new_run {
            let rl = line_text.len() - run_start;
            if rl > 0 {
                runs.push(make_run(
                    rl,
                    cur_bold,
                    cur_italic,
                    cur_fg,
                    cur_inverse,
                    cur_inv_bg,
                    cur_underline,
                ));
            }
            run_start = line_text.len();
            cur_fg = ef;
            cur_inv_bg = eb;
            cur_bold = is_b;
            cur_italic = is_i;
            cur_underline = is_u;
            cur_inverse = is_inv;
        }

        if cell.c == '\t' {
            let ns = ((ci / 8) + 1) * 8 - ci;
            for _ in 0..ns {
                line_text.push(' ');
            }
        } else {
            line_text.push(cell.c);
        }
    }

    let rl = line_text.len() - run_start;
    if rl > 0 {
        runs.push(make_run(
            rl,
            cur_bold,
            cur_italic,
            cur_fg,
            cur_inverse,
            cur_inv_bg,
            cur_underline,
        ));
    }

    if line_text.len() < num_cols {
        let pad = num_cols - line_text.len();
        line_text.extend(std::iter::repeat(' ').take(pad));
        runs.push(TextRun {
            len: pad,
            font: pick_font(false, false),
            color: rgb(TERM_FG).into(),
            background_color: None,
            underline: None,
            strikethrough: None,
        });
    }

    (line_text, runs)
}
