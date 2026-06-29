use std::num::NonZeroUsize;
use std::sync::Arc;

use alacritty_terminal::term::cell::Flags;
use gpui::ShapedLine;
use lru::LruCache;
use parking_lot::Mutex;

#[derive(Clone)]
pub(crate) struct CellSnap {
    pub c: char,
    pub fg: u32,
    pub bg: u32,
    pub flags: Flags,
    pub custom_bg: bool,
}

#[derive(Clone)]
pub(crate) struct RowSnapshot {
    pub cells: Vec<CellSnap>,
    pub hash: u64,
    /// True if any cell needs a background quad (custom bg or inverse).
    pub has_bg: bool,
}

impl Default for RowSnapshot {
    fn default() -> Self {
        Self {
            cells: Vec::new(),
            hash: u64::MAX,
            has_bg: false,
        }
    }
}

pub(crate) struct RenderCache {
    pub rows: Vec<RowSnapshot>,
    /// Keyed by row content hash so scrolled-back lines reuse their shaping.
    pub shaped: LruCache<u64, ShapedLine>,
    pub cols: usize,
    pub rows_count: usize,
}

impl Default for RenderCache {
    fn default() -> Self {
        Self {
            rows: Vec::new(),
            shaped: LruCache::new(NonZeroUsize::new(1024).unwrap()),
            cols: 0,
            rows_count: 0,
        }
    }
}

impl RenderCache {
    pub fn resize(&mut self, cols: usize, rows: usize) {
        self.cols = cols;
        self.rows_count = rows;
        self.rows = vec![RowSnapshot::default(); rows];
        // Keep the shaped LRU — identical lines after resize still hit.
    }

    pub fn clear_all(&mut self) {
        self.rows.clear();
        self.shaped.clear();
        self.cols = 0;
        self.rows_count = 0;
    }
}

pub(crate) fn hash_row(cells: &[CellSnap]) -> u64 {
    use std::hash::Hasher;
    let mut h = rustc_hash::FxHasher::default();
    for c in cells {
        h.write_u32(c.c as u32);
        h.write_u32(c.fg);
        h.write_u32(c.bg);
        h.write_u16(c.flags.bits() as u16);
    }
    h.finish()
}

pub(crate) type SharedRenderCache = Arc<Mutex<RenderCache>>;
