// Just Enough Items for Horizon: the game's tooltips over JEI's overlays.
//
// In Java the screen's tooltips are deferred to the end of the frame
// (`GuiGraphicsExtractor.setTooltipForNextFrame`, drawn after
// `ScreenEvent.Render.Post`), so they show over everything JEI draws there.
// Horizon draws them inside the container screen, before the overlays. Each
// tooltip the screen draws is noted here as its run of vertices (the hooks
// on a tooltip box and its text, and on a stack's whole tooltip); the
// overlays take those runs out of the batch before they draw and put them back on
// top afterwards.
use horizon_client::hud::{Blend, HudBatch, HudVertex};
use std::cell::{Cell, RefCell};
use std::ops::Range;

/// More runs than a screen ever draws: something kept noting without a
/// screen to take them; start again.
const MAX_RUNS: usize = 64;

thread_local! {
    /// A container screen is drawing: its tooltips are noted.
    static ARMED: Cell<bool> = const { Cell::new(false) };
    /// Tooltip painters open (a stack's tooltip paints its bundle's name in
    /// one of its own) and where the outermost one began.
    static OPEN: Cell<(u32, u32)> = const { Cell::new((0, 0)) };
    /// The runs of the tooltips drawn so far: start and end vertex.
    static RUNS: RefCell<Vec<(u32, u32)>> = const { RefCell::new(Vec::new()) };
}

/// The game's tooltips cut out of the batch, with their blend runs (from 0).
#[derive(Default)]
pub(super) struct Tooltips {
    vertices: Vec<HudVertex>,
    blends: Vec<(Range<u32>, Blend)>,
}

/// A container screen (the inventory, a container's, the creative one)
/// starts drawing.
pub(super) fn screen_begins() {
    ARMED.with(|a| a.set(true));
    OPEN.with(|o| o.set((0, 0)));
    RUNS.with(|r| r.borrow_mut().clear());
}

/// A tooltip painter starts, the batch holding `at` vertices.
pub(super) fn tooltip_begins(at: usize) {
    if !ARMED.with(Cell::get) {
        return;
    }
    OPEN.with(|o| {
        let (depth, start) = o.get();
        o.set(if depth == 0 { (1, at as u32) } else { (depth + 1, start) });
    });
}

/// A tooltip painter returns, the batch holding `at` vertices.
pub(super) fn tooltip_ends(at: usize) {
    if !ARMED.with(Cell::get) {
        return;
    }
    let (depth, start) = OPEN.with(Cell::get);
    if depth == 0 {
        return;
    }
    OPEN.with(|o| o.set((depth - 1, start)));
    if depth == 1 && at as u32 > start {
        RUNS.with(|r| {
            let mut runs = r.borrow_mut();
            if runs.len() >= MAX_RUNS {
                runs.clear();
            }
            runs.push((start, at as u32));
        });
    }
}

/// The overlays are off: the screen's tooltips stay where they are.
pub(super) fn forget() {
    ARMED.with(|a| a.set(false));
    OPEN.with(|o| o.set((0, 0)));
    RUNS.with(|r| r.borrow_mut().clear());
}

/// Takes the tooltips the container screen drew this frame out of `batch`
/// (what was drawn between them closes up), to go back on top with [`put_back`].
pub(super) fn take(batch: &mut HudBatch) -> Tooltips {
    ARMED.with(|a| a.set(false));
    OPEN.with(|o| o.set((0, 0)));
    let mut runs = RUNS.with(|r| std::mem::take(&mut *r.borrow_mut()));
    let len = batch.vertices.len() as u32;
    // Only the screen's own: never the HUD under it, never past the batch.
    runs.retain(|&(s, e)| s < e && s >= batch.blur_at && e <= len);
    runs.sort_unstable();
    let mut cuts: Vec<(u32, u32)> = Vec::new();
    for (s, e) in runs {
        match cuts.last_mut() {
            Some(last) if s <= last.1 => last.1 = last.1.max(e),
            _ => cuts.push((s, e)),
        }
    }
    let mut out = Tooltips::default();
    if cuts.is_empty() {
        return out;
    }
    // Blend runs split at the cuts' edges: each piece then lies inside one
    // cut or outside them all.
    let mut pieces = Vec::with_capacity(batch.blends.len());
    for (r, blend) in std::mem::take(&mut batch.blends) {
        let mut edges = vec![r.start];
        for &(s, e) in &cuts {
            for x in [s, e] {
                if x > r.start && x < r.end {
                    edges.push(x);
                }
            }
        }
        edges.push(r.end);
        for w in edges.windows(2) {
            if w[0] < w[1] {
                pieces.push((w[0]..w[1], blend));
            }
        }
    }
    let old = std::mem::take(&mut batch.vertices);
    let mut kept = Vec::with_capacity(old.len());
    // Where each cut starts in `out`.
    let mut bases = Vec::with_capacity(cuts.len());
    let mut at = 0usize;
    for &(s, e) in &cuts {
        kept.extend_from_slice(&old[at..s as usize]);
        bases.push(out.vertices.len() as u32);
        out.vertices.extend_from_slice(&old[s as usize..e as usize]);
        at = e as usize;
    }
    kept.extend_from_slice(&old[at..]);
    batch.vertices = kept;
    for (r, blend) in pieces {
        if let Some(i) = cuts.iter().position(|&(s, e)| r.start >= s && r.end <= e) {
            let shift = cuts[i].0 - bases[i];
            out.blends.push((r.start - shift..r.end - shift, blend));
        } else {
            let shift: u32 = cuts.iter().filter(|&&(_, e)| e <= r.start).map(|&(s, e)| e - s).sum();
            batch.blends.push((r.start - shift..r.end - shift, blend));
        }
    }
    out
}

/// Draws the tooltips [`take`] cut out over everything drawn since.
pub(super) fn put_back(batch: &mut HudBatch, tooltips: Tooltips) {
    let base = batch.vertices.len() as u32;
    batch.vertices.extend_from_slice(&tooltips.vertices);
    for (r, blend) in tooltips.blends {
        batch.blends.push((r.start + base..r.end + base, blend));
    }
}
