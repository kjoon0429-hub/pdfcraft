//! Dragging pages between documents: thumbnails picked up in the Pages panel and held over
//! another document's tab open that tab with its Pages panel (spring-loaded tabs); dropped on
//! that panel, they are copied into the gap they are dropped on. Held over their own tab again,
//! they move within it as if they had never left.

use pdfcraft_engine::{DocId, Edit, FieldConflicts};

use crate::{PdfCraftApp, RightPanel};

/// How long pages must rest on a tab before it opens, in seconds: long enough to cross the
/// strip without opening every tab on the way.
pub(crate) const SPRING_DELAY: f64 = 0.4;

/// Pages carried out of their document's Pages panel: that document and the pages, as they
/// were numbered when the drag began.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageCarry {
    pub from: DocId,
    pub pages: Vec<usize>,
}

/// Seconds still to wait, at `now`, before the tab pages rest on since `since` opens.
fn spring_left(since: f64, now: f64) -> f64 {
    let left = SPRING_DELAY - (now - since);
    if left.is_finite() { left.max(0.0) } else { 0.0 }
}

impl PdfCraftApp {
    /// Pages are being dragged out of a Pages panel: carried across tabs, or still in the
    /// active tab's panel (where they can be carried from).
    pub(crate) fn dragging_pages(&self, ctx: &egui::Context) -> bool {
        ctx.input(|i| i.pointer.primary_down())
            && (self.page_carry.is_some() || self.active.and_then(|i| self.views.get(i)).is_some_and(|v| v.panel_drag.is_some()))
    }

    /// A carry whose button came up without landing on a Pages panel ends. Called once a frame,
    /// before the panels: the frame the button comes up in still belongs to the drop.
    pub(crate) fn end_stale_page_carry(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| !i.pointer.primary_down() && !i.pointer.primary_released()) {
            self.page_carry = None;
            self.tab_spring = None;
        }
    }

    /// Pages being dragged rest on document tab `over` (or on none). Once they have rested
    /// there for [`SPRING_DELAY`], that tab becomes the active one with its Pages panel open,
    /// and the drag continues there.
    pub(crate) fn spring_tab(&mut self, ctx: &egui::Context, over: Option<usize>) {
        let Some(target) = over.and_then(|i| self.views.get(i)).map(|v| v.id) else {
            self.tab_spring = None;
            return;
        };
        let now = ctx.input(|i| i.time);
        let since = match self.tab_spring {
            Some((id, since)) if id == target => since,
            _ => {
                self.tab_spring = Some((target, now));
                now
            }
        };
        let left = spring_left(since, now);
        if left > 0.0 {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(left));
            return;
        }
        self.tab_spring = None;
        // The drag leaves the panel it began in: the app carries it from here.
        if self.page_carry.is_none() {
            let Some((index, from)) = self.active_ids() else { return };
            let Some(pages) = self.views.get_mut(index).and_then(|v| v.panel_drag.take()) else { return };
            self.page_carry = Some(PageCarry { from, pages });
        }
        let Some(index) = self.views.iter().position(|v| v.id == target) else { return };
        self.active = Some(index);
        if self.right != Some(RightPanel::Pages) {
            self.choose_right_panel(Some(RightPanel::Pages));
        }
    }

    /// The carried pages were dropped on the active document's Pages panel, in `gap` (0 =
    /// before the first page). Into another document they are copied, as Paste would (fields
    /// whose names collide are renamed); back in their own document they move.
    pub(crate) fn drop_page_carry(&mut self, gap: usize) {
        let Some(carry) = self.page_carry.take() else { return };
        self.tab_spring = None;
        let Some((index, id)) = self.active_ids() else { return };
        if carry.from == id {
            if let Some(view) = self.views.get_mut(index) {
                crate::panels::queue_move(view, carry.pages, gap);
            }
            return;
        }
        let Some((name, bytes)) = self.session.get(carry.from).map(|d| (d.name.clone(), d.bytes.clone())) else {
            // The source closed mid-drag (it can't from the UI, but nothing promises that).
            return;
        };
        let count = carry.pages.len();
        if self.apply_edit(Edit::InsertPagesFrom { name, bytes, pages: Some(carry.pages), at: gap, field_conflicts: FieldConflicts::Rename })
            && let Some(view) = self.views.get_mut(index)
        {
            view.select_pages(&(gap..gap.saturating_add(count)).collect::<Vec<_>>());
            let what = if count == 1 { tl!("1 page").to_string() } else { crate::i18n::fmt(tl!("{n} pages"), &[("n", &count.to_string())]) };
            self.notify_fmt("Copied {what}", &[("what", &what)]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_spring_waits_its_delay_and_never_goes_negative_or_nan() {
        assert_eq!(spring_left(10.0, 10.0), SPRING_DELAY);
        assert!(spring_left(10.0, 10.1) > 0.0);
        assert_eq!(spring_left(10.0, 10.0 + SPRING_DELAY), 0.0);
        assert_eq!(spring_left(10.0, 99.0), 0.0);
        assert_eq!(spring_left(f64::NAN, 1.0), 0.0);
        assert_eq!(spring_left(0.0, f64::INFINITY), 0.0);
    }
}
