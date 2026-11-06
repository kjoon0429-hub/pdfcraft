//! Dragging Pages panel thumbnails onto another document's tab: resting there opens that tab
//! with its Pages panel, and dropping on that panel copies the pages into the gap.

use egui::{Pos2, Rect, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use pdfcraft_render::{PageRenderer, RenderRequest, RequestKind};
use pdfcraft_ui_egui::{PdfCraftApp, RightPanel};

/// An `n`-page document; page `i` shows `{tag}{i+1}`.
fn fixture(tag: &str, n: usize) -> Vec<u8> {
    let mut objs: Vec<String> = vec!["<< /Type /Catalog /Pages 2 0 R >>".into()];
    let kids: Vec<String> = (0..n).map(|i| format!("{} 0 R", 4 + 2 * i)).collect();
    objs.push(format!("<< /Type /Pages /Kids [{}] /Count {n} /MediaBox [0 0 200 300] >>", kids.join(" ")));
    objs.push("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".into());
    for i in 0..n {
        objs.push(format!("<< /Type /Page /Parent 2 0 R /Contents {} 0 R /Resources << /Font << /F1 3 0 R >> >> >>", 5 + 2 * i));
        let body = format!("BT /F1 24 Tf 20 150 Td ({tag}{}) Tj ET", i + 1);
        objs.push(format!("<< /Length {} >>\nstream\n{body}\nendstream", body.len()));
    }
    let mut out = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for (i, o) in objs.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{o}\nendobj\n", i + 1).as_bytes());
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes());
    for o in offsets {
        out.extend_from_slice(format!("{o:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objs.len() + 1).as_bytes());
    out
}

/// Two tabs, `a.pdf` (A1–A3, active) and `b.pdf` (B1–B2), with the Pages panel open.
fn two_tabs() -> Harness<'static, PdfCraftApp> {
    let mut h = Harness::builder().with_size(vec2(1400.0, 900.0)).build_eframe(|_cc| {
        let mut app = PdfCraftApp::new();
        app.set_option("language", "en").unwrap();
        app.open_bytes("a.pdf", None, fixture("A", 3)).expect("a.pdf opens");
        app.open_bytes("b.pdf", None, fixture("B", 2)).expect("b.pdf opens");
        app.active = Some(0);
        app.right = Some(RightPanel::Pages);
        app
    });
    h.run_steps(4);
    h
}

/// Each page's text in tab `tab`, read back from the document's current bytes.
fn texts(app: &PdfCraftApp, tab: usize) -> Vec<String> {
    let doc = app.session.get(app.views[tab].id).unwrap();
    let mut r = PageRenderer::new(doc.bytes.clone(), Default::default());
    (0..r.page_count())
        .map(|p| {
            let out = r.render(RenderRequest { page: p, kind: RequestKind::Text, scale: 1.0, ..Default::default() });
            out.text.map(|t| t.plain_text().trim().to_string()).unwrap_or_default()
        })
        .collect()
}

/// A Pages panel thumbnail: the rightmost node with that label (the page view labels pages too).
fn thumb(h: &Harness<'static, PdfCraftApp>, label: &str) -> Rect {
    h.get_all_by_label(label).map(|n| n.rect()).max_by(|a, b| a.left().total_cmp(&b.left())).expect("the thumbnail")
}

fn tab(h: &Harness<'static, PdfCraftApp>, name: &str) -> Pos2 {
    h.get_by_label(name).rect().center()
}

/// Press on `from` and move, in a few frames, to `to`, keeping the button down.
fn grab_and_move(h: &mut Harness<'static, PdfCraftApp>, from: Pos2, to: Pos2) {
    h.hover_at(from);
    h.run_steps(1);
    h.drag_at(from);
    h.run_steps(1);
    move_to(h, from, to);
}

fn move_to(h: &mut Harness<'static, PdfCraftApp>, from: Pos2, to: Pos2) {
    for k in 1..=5 {
        h.hover_at(from + (to - from) * (k as f32 / 5.0));
        h.run_steps(1);
    }
}

/// Keep the pointer on `at` until tab `want` is the active one (or give up). Each step advances
/// the harness clock, so the spring delay passes without sleeping.
fn rest_until_active(h: &mut Harness<'static, PdfCraftApp>, at: Pos2, want: usize) {
    for _ in 0..40 {
        if h.state().active == Some(want) {
            return;
        }
        h.hover_at(at);
        h.step();
    }
    panic!("tab {want} never opened; active = {:?}", h.state().active);
}

#[test]
fn pages_rested_on_another_tab_open_it_and_are_copied_where_dropped() {
    let mut h = two_tabs();
    let from = thumb(&h, "Page 2").center();
    let over = tab(&h, "b.pdf");
    grab_and_move(&mut h, from, over);
    assert_eq!(h.state().active, Some(0), "passing over a tab doesn't open it");
    rest_until_active(&mut h, over, 1);
    assert_eq!(h.state().right, Some(RightPanel::Pages), "the Pages panel opens with the tab");
    assert!(h.state().page_carry.is_some());
    // Drop on the lower half of B1: the gap between B1 and B2.
    let b1 = thumb(&h, "Page 1");
    let to = b1.center() + vec2(0.0, b1.height() * 0.3);
    move_to(&mut h, over, to);
    h.drop_at(to);
    h.run_steps(4);
    let s = h.state();
    assert_eq!(texts(s, 1), ["B1", "A2", "B2"], "copied into the gap");
    assert_eq!(texts(s, 0), ["A1", "A2", "A3"], "the source keeps its pages");
    assert!(s.session.get(s.views[1].id).unwrap().can_undo().is_some_and(|l| l.starts_with("Insert pages from")));
    assert_eq!(s.views[1].selected.iter().copied().collect::<Vec<_>>(), [1], "the copy is selected");
    assert!(s.page_carry.is_none());
    assert!(!s.session.get(s.views[0].id).unwrap().dirty);
}

#[test]
fn carried_pages_let_go_off_the_panel_copy_nothing() {
    let mut h = two_tabs();
    let from = thumb(&h, "Page 1").center();
    let over = tab(&h, "b.pdf");
    grab_and_move(&mut h, from, over);
    rest_until_active(&mut h, over, 1);
    // Released on the tab strip, not on the panel.
    h.drop_at(over);
    h.run_steps(4);
    let s = h.state();
    assert_eq!(texts(s, 1), ["B1", "B2"]);
    assert!(s.page_carry.is_none(), "the carry ends with the button");
    assert!(!s.session.get(s.views[1].id).unwrap().dirty);
}

#[test]
fn carried_back_to_their_own_tab_pages_move_instead() {
    let mut h = two_tabs();
    let from = thumb(&h, "Page 2").center();
    let (a, b) = (tab(&h, "a.pdf"), tab(&h, "b.pdf"));
    grab_and_move(&mut h, from, b);
    rest_until_active(&mut h, b, 1);
    move_to(&mut h, b, a);
    rest_until_active(&mut h, a, 0);
    let a1 = thumb(&h, "Page 1");
    let to = a1.center_top() + vec2(0.0, 4.0);
    move_to(&mut h, a, to);
    h.drop_at(to);
    h.run_steps(4);
    let s = h.state();
    assert_eq!(texts(s, 0), ["A2", "A1", "A3"]);
    assert_eq!(s.session.get(s.views[0].id).unwrap().can_undo(), Some("Move page"));
    assert_eq!(texts(s, 1), ["B1", "B2"]);
}

#[test]
fn a_panel_drag_released_over_the_tab_strip_moves_nothing() {
    let mut h = two_tabs();
    let from = thumb(&h, "Page 3").center();
    // Its own (active) tab never springs; letting go there is off the panel.
    let own = tab(&h, "a.pdf");
    grab_and_move(&mut h, from, own);
    h.drop_at(own);
    h.run_steps(4);
    let s = h.state();
    assert_eq!(texts(s, 0), ["A1", "A2", "A3"]);
    assert!(s.views[0].panel_drag.is_none());
    assert_eq!(s.active, Some(0));
}
