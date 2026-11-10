//! The vendored winprint's patch (vendor/README.md): Windows prints each page at the printer's
//! resolution, so images aren't printed at 96 dpi. The printing itself needs a Windows printer;
//! this keeps the patch, and the `[patch]` that puts it in the build, from being lost.

fn read(path: &str) -> String {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::read_to_string(root.join(path)).unwrap_or_else(|e| panic!("{path}: {e}")).replace("\r\n", "\n")
}

#[test]
fn windows_prints_pages_at_the_printers_resolution() {
    let win_pdf = read("vendor/winprint/src/printer/win_pdf.rs");
    assert!(win_pdf.contains("PdfCraft patch: Windows.Data.Pdf rasterizes a page's images"));
    assert!(win_pdf.contains("let scale = page_scale(context.raster_dpi, pdf_size.Width, pdf_size.Height);"));
    assert!(win_pdf.contains("render_options.DestinationWidth = (pdf_size.Width * scale).round() as u32;"));
    assert!(win_pdf.contains("d2d_context.SetTransform(&scaled(1.0 / scale));"));
    assert!(win_pdf.contains("Some(&render_options)"), "the page is rendered with the scaled size");
    assert!(read("vendor/winprint/src/printer/dxgi.rs").contains("raster_dpi: raster_dpi as f32,"));
    let manifest = read("Cargo.toml");
    assert!(manifest.contains("[patch.\"https://github.com/storytold/winprint.rs\"]\nwinprint = { path = \"vendor/winprint\" }"));
}
