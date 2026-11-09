//! This fork's interface changes: scroll bars always on show, a page scroll bar one page at a time,
//! Fit one full page as a standing choice, F4 opening the page thumbnails, the selected text's own
//! right-click menu, and the keys of the thumbnails' menu.

use egui::{Pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use pdfcraft_ui_egui::{PdfCraftApp, RightPanel};

/// An `n`-page document; page `i` shows `{tag}{i+1} word`.
fn fixture(tag: &str, n: usize) -> Vec<u8> {
    let mut objs: Vec<String> = vec!["<< /Type /Catalog /Pages 2 0 R >>".into()];
    let kids: Vec<String> = (0..n).map(|i| format!("{} 0 R", 4 + 2 * i)).collect();
    objs.push(format!("<< /Type /Pages /Kids [{}] /Count {n} /MediaBox [0 0 300 400] >>", kids.join(" ")));
    objs.push("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".into());
    for i in 0..n {
        objs.push(format!("<< /Type /Page /Parent 2 0 R /Contents {} 0 R /Resources << /Font << /F1 3 0 R >> >> >>", 5 + 2 * i));
        let body = format!("BT /F1 24 Tf 20 300 Td ({tag}{} word) Tj ET", i + 1);
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

fn harness(settings: &'static str, open: impl FnOnce(&mut PdfCraftApp) + 'static) -> Harness<'static, PdfCraftApp> {
    let mut h = Harness::builder().with_size(vec2(1400.0, 900.0)).build_eframe(move |_cc| {
        let mut app = PdfCraftApp::new();
        app.restore(settings);
        app.set_option("language", "en").unwrap();
        open(&mut app);
        app
    });
    h.run_steps(4);
    h
}

fn one_doc() -> Harness<'static, PdfCraftApp> {
    harness("{}", |app| {
        app.open_bytes("a.pdf", None, fixture("A", 4)).unwrap();
    })
}

fn right_click(h: &mut Harness<'static, PdfCraftApp>, at: Pos2) {
    h.hover_at(at);
    h.run_steps(1);
    let button = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Secondary, pressed, modifiers: egui::Modifiers::NONE };
    h.event(button(true));
    h.event(button(false));
    h.run_steps(3);
}

/// Wait for the page's text layer, and select its first word ("A1").
fn select_first_word(h: &mut Harness<'static, PdfCraftApp>) -> Pos2 {
    for _ in 0..200 {
        if h.state().views[0].glyph_screen_pos(0, 0).is_some() {
            break;
        }
        h.run_steps(1);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    h.state_mut().views[0].select_text(0, 0, 1);
    h.run_steps(3);
    h.state().views[0].glyph_screen_pos(0, 0).expect("the page's text")
}

#[test]
fn scroll_bars_stay_on_show() {
    let h = one_doc();
    let scroll = h.ctx.global_style().spacing.scroll;
    assert!(scroll.floating && scroll.dormant_handle_opacity > 0.0, "visible while the pointer is elsewhere");
    assert!(scroll.floating_width >= 6.0);
}

#[test]
fn one_page_at_a_time_a_page_scroll_bar_goes_to_the_page_at_its_height() {
    let mut h = one_doc();
    assert!(h.query_by_label("Page scrollbar").is_none(), "scrolling continuously, the page's own scroll bar shows");
    h.state_mut().execute("view.fit_one_page");
    h.run_steps(4);
    let track = h.get_by_label("Page scrollbar").rect();
    let at = |y: f32| Pos2::new(track.center().x, y);
    h.drag_at(at(track.top() + 6.0));
    h.run_steps(1);
    for k in 1..=4 {
        h.hover_at(at(track.top() + (track.height() - 6.0) * (k as f32 / 4.0)));
        h.run_steps(1);
    }
    h.drop_at(at(track.bottom() - 2.0));
    h.run_steps(2);
    assert_eq!(h.state().views[0].current, 3, "dragged to the bottom: the last page");
    assert!(h.state().views[0].selected_text().is_none(), "and no text selected on the way");
    h.state_mut().execute("view.fit_width_scrolling");
    h.run_steps(4);
    assert!(h.query_by_label("Page scrollbar").is_none());
}

#[test]
fn fit_one_full_page_holds_for_every_tab_and_later_documents_and_the_next_start() {
    let mut h = harness("{}", |app| {
        app.open_bytes("a.pdf", None, fixture("A", 2)).unwrap();
        app.open_bytes("b.pdf", None, fixture("B", 2)).unwrap();
    });
    assert!(!h.state().views[0].fits_one_page());
    h.state_mut().execute("view.fit_one_page");
    h.run_steps(2);
    assert!(h.state().views.iter().all(|v| v.fits_one_page()), "every tab");
    h.state_mut().open_bytes("c.pdf", None, fixture("C", 2)).unwrap();
    h.run_steps(2);
    assert!(h.state().views[2].fits_one_page(), "a document opened later");

    let saved = h.state().persist();
    let mut next = PdfCraftApp::new();
    next.restore(&saved);
    next.open_bytes("d.pdf", None, fixture("D", 2)).unwrap();
    assert!(next.views[0].fits_one_page(), "the next start");

    h.state_mut().execute("view.fit_width_scrolling");
    h.state_mut().open_bytes("e.pdf", None, fixture("E", 2)).unwrap();
    h.run_steps(2);
    assert!(h.state().views.iter().all(|v| !v.fits_one_page()), "until another standing choice");
}

/// Over how the file was last shown; only its page comes back.
#[test]
fn fit_one_full_page_wins_over_the_files_last_view_but_keeps_its_page() {
    let dir = std::env::temp_dir().join(format!("pdfcraft-fork-ui-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("remembered.pdf");
    std::fs::write(&path, fixture("R", 3)).unwrap();
    let path = std::fs::canonicalize(&path).unwrap().to_string_lossy().into_owned();
    let settings = serde_json::json!({
        "default_layout": "single",
        "default_zoom": "fit-page",
        "last_views": [{ "path": path, "page": 1, "zoom": "150%", "display": "continuous", "cover": false }],
    })
    .to_string();
    let mut app = PdfCraftApp::new();
    app.restore(&settings);
    app.open_bytes("remembered.pdf", Some(path), fixture("R", 3)).unwrap();
    assert!(app.views[0].fits_one_page());
    assert_eq!(app.views[0].current, 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn f4_always_opens_the_page_thumbnails() {
    let mut h = one_doc();
    h.state_mut().choose_right_panel(Some(RightPanel::Bookmarks));
    h.run_steps(2);
    h.state_mut().execute("view.side_panel");
    h.run_steps(2);
    assert_eq!(h.state().right, None, "open: F4 closes it");
    h.state_mut().execute("view.side_panel");
    h.run_steps(2);
    assert_eq!(h.state().right, Some(RightPanel::Pages), "not the Bookmarks open last");
}

#[test]
fn selected_text_shows_no_menu_until_right_clicked_and_then_only_its_own() {
    let mut h = one_doc();
    let on_text = select_first_word(&mut h);
    assert!(h.query_by_label("Replace text").is_none(), "nothing pops up after selecting");

    right_click(&mut h, on_text);
    h.get_by_label("Replace text");
    h.get_by_label("Strikethrough");
    assert!(h.query_by_label("Add bookmark").is_none(), "not the page's menu");
    assert!(h.state().views[0].selected_text().is_some(), "the right-click keeps the selection");

    h.get_by_label("Underline").click();
    h.run_steps(4);
    let s = h.state();
    let doc = s.session.get(s.views[0].id).unwrap();
    assert_eq!(doc.info.annotations.iter().map(|a| a.subtype.as_str()).collect::<Vec<_>>(), ["Underline"]);
    assert!(s.views[0].selected_text().is_none());
}

#[test]
fn off_the_selected_text_a_right_click_opens_the_pages_menu() {
    let mut h = one_doc();
    let on_text = select_first_word(&mut h);
    right_click(&mut h, on_text + vec2(0.0, 150.0));
    h.get_by_label("Add bookmark");
    assert!(h.query_by_label("Replace text").is_none());
}

fn with_pages_panel() -> Harness<'static, PdfCraftApp> {
    let mut h = one_doc();
    h.state_mut().choose_right_panel(Some(RightPanel::Pages));
    h.run_steps(4);
    h
}

fn thumb(h: &Harness<'static, PdfCraftApp>, label: &str) -> Pos2 {
    h.get_all_by_label(label).map(|n| n.rect()).max_by(|a, b| a.left().total_cmp(&b.left())).expect("the thumbnail").center()
}

#[test]
fn x_extracts_with_the_pointer_over_the_thumbnails_and_only_there() {
    let mut h = with_pages_panel();
    let over = thumb(&h, "Page 1");
    h.hover_at(Pos2::new(300.0, 500.0));
    h.run_steps(1);
    h.key_press(egui::Key::X);
    h.run_steps(2);
    assert!(h.state().dialog.is_none(), "over the page, X is nothing");
    h.hover_at(over);
    h.run_steps(1);
    h.key_press(egui::Key::X);
    h.run_steps(3);
    assert!(h.state().dialog.is_some(), "the Extract dialog");
}

#[test]
fn clipboard_keys_copy_and_paste_the_pages_once_the_panel_has_focus() {
    let mut h = with_pages_panel();
    h.get_all_by_label("Page 2").max_by(|a, b| a.rect().left().total_cmp(&b.rect().left())).expect("the thumbnail").click();
    h.run_steps(3);
    h.event(egui::Event::Copy);
    h.run_steps(3);
    assert!(h.state().page_clipboard.is_some(), "copied");
    h.event(egui::Event::Paste("anything".into()));
    h.run_steps(4);
    let s = h.state();
    assert_eq!(s.session.get(s.views[0].id).unwrap().info.pages.len(), 5, "pasted");
}

/// The menu says so, Extract with X.
#[test]
fn the_thumbnails_menu_shows_its_keys() {
    let mut h = with_pages_panel();
    let t = thumb(&h, "Page 1");
    right_click(&mut h, t);
    h.get_by_label_contains("Extract pages");
    h.get_by_label_contains("Delete pages");
    h.get_by_label_contains("Paste after");
}
