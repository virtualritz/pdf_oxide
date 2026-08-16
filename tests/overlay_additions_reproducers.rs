//! Reproducers for three defects in the `overlay_additions` path, i.e. what
//! `DocumentEditor::edit_page` and `add_image_bytes_to_page` produce when the
//! page came from an existing document.
//!
//! Each test asserts through the library's own reader, which is CTM-aware and
//! correct, so a failure is the writer's.
//!
//! All three fail on v0.3.77.

use pdf_oxide::document::PdfDocument;
use pdf_oxide::editor::{DocumentEditor, SaveOptions};
use pdf_oxide::elements::{FontSpec, TextContent, TextStyle};
use pdf_oxide::geometry::Rect;

/// A US Letter page whose content stream sets a y-flip and never restores it,
/// compensating in the text matrix so the text still reads upright.
///
/// This is what Word, LibreOffice and a good many enterprise exporters emit,
/// and it leaves the CTM non-identity at the end of the stream.
const FLIPPED: &str = "1 0 0 -1 0 792 cm\nBT /F1 12 Tf 1 0 0 -1 72 -720 Tm (original) Tj ET\n";

/// The smallest one-page PDF carrying `content`, with a Helvetica resource.
fn page(content: &str) -> Vec<u8> {
    let objects = [
        "<</Type/Catalog/Pages 2 0 R>>".to_string(),
        "<</Type/Pages/Kids[3 0 R]/Count 1>>".to_string(),
        "<</Type/Page/Parent 2 0 R/MediaBox[0 0 612 792]\
         /Resources<</Font<</F1 5 0 R>>>>/Contents 4 0 R>>"
            .to_string(),
        format!("<</Length {}>>\nstream\n{content}endstream", content.len()),
        "<</Type/Font/Subtype/Type1/BaseFont/Helvetica>>".to_string(),
    ];

    let mut pdf = b"%PDF-1.7\n".to_vec();
    let offsets: Vec<usize> = objects
        .iter()
        .enumerate()
        .map(|(index, object)| {
            let offset = pdf.len();
            pdf.extend_from_slice(format!("{} 0 obj\n{object}\nendobj\n", index + 1).as_bytes());
            offset
        })
        .collect();

    let startxref = pdf.len();
    pdf.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
    pdf.extend_from_slice(b"0000000000 65535 f \n");
    offsets
        .iter()
        .for_each(|offset| pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes()));
    pdf.extend_from_slice(
        format!(
            "trailer\n<</Size {}/Root 1 0 R>>\nstartxref\n{startxref}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    pdf
}

/// A 16x16 RGBA PNG. Deliberately over the 8x8 floor `ImageExtractFilter`
/// applies by default, and carrying real alpha so the `/SMask` path is
/// exercised too.
fn png() -> Vec<u8> {
    const BYTES: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x10, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0xF3, 0xFF, 0x61, 0x00, 0x00, 0x00, 0x20, 0x63, 0x48, 0x52, 0x4D, 0x00, 0x00, 0x7A, 0x26,
        0x00, 0x00, 0x80, 0x84, 0x00, 0x00, 0xFA, 0x00, 0x00, 0x00, 0x80, 0xE8, 0x00, 0x00, 0x75,
        0x30, 0x00, 0x00, 0xEA, 0x60, 0x00, 0x00, 0x3A, 0x98, 0x00, 0x00, 0x17, 0x70, 0x9C, 0xBA,
        0x51, 0x3C, 0x00, 0x00, 0x00, 0x06, 0x62, 0x4B, 0x47, 0x44, 0x00, 0xFF, 0x00, 0xFF, 0x00,
        0xFF, 0xA0, 0xBD, 0xA7, 0x93, 0x00, 0x00, 0x00, 0x07, 0x74, 0x49, 0x4D, 0x45, 0x07, 0xEA,
        0x08, 0x0A, 0x0E, 0x31, 0x10, 0xF5, 0xB2, 0x03, 0x3D, 0x00, 0x00, 0x00, 0x25, 0x74, 0x45,
        0x58, 0x74, 0x64, 0x61, 0x74, 0x65, 0x3A, 0x63, 0x72, 0x65, 0x61, 0x74, 0x65, 0x00, 0x32,
        0x30, 0x32, 0x36, 0x2D, 0x30, 0x38, 0x2D, 0x31, 0x30, 0x54, 0x31, 0x34, 0x3A, 0x34, 0x39,
        0x3A, 0x31, 0x36, 0x2B, 0x30, 0x30, 0x3A, 0x30, 0x30, 0x4E, 0x6A, 0x0C, 0xB8, 0x00, 0x00,
        0x00, 0x25, 0x74, 0x45, 0x58, 0x74, 0x64, 0x61, 0x74, 0x65, 0x3A, 0x6D, 0x6F, 0x64, 0x69,
        0x66, 0x79, 0x00, 0x32, 0x30, 0x32, 0x36, 0x2D, 0x30, 0x38, 0x2D, 0x31, 0x30, 0x54, 0x31,
        0x34, 0x3A, 0x34, 0x39, 0x3A, 0x31, 0x36, 0x2B, 0x30, 0x30, 0x3A, 0x30, 0x30, 0x3F, 0x37,
        0xB4, 0x04, 0x00, 0x00, 0x00, 0x28, 0x74, 0x45, 0x58, 0x74, 0x64, 0x61, 0x74, 0x65, 0x3A,
        0x74, 0x69, 0x6D, 0x65, 0x73, 0x74, 0x61, 0x6D, 0x70, 0x00, 0x32, 0x30, 0x32, 0x36, 0x2D,
        0x30, 0x38, 0x2D, 0x31, 0x30, 0x54, 0x31, 0x34, 0x3A, 0x34, 0x39, 0x3A, 0x31, 0x36, 0x2B,
        0x30, 0x30, 0x3A, 0x30, 0x30, 0x68, 0x22, 0x95, 0xDB, 0x00, 0x00, 0x00, 0x72, 0x49, 0x44,
        0x41, 0x54, 0x38, 0xCB, 0xCD, 0x93, 0xD1, 0x09, 0xC0, 0x20, 0x0C, 0x44, 0x9F, 0x1D, 0xC2,
        0xE5, 0x84, 0x4E, 0xE1, 0x3C, 0x8E, 0xA5, 0xE3, 0x5C, 0x7F, 0x2C, 0x44, 0x41, 0x30, 0xF6,
        0xA7, 0x81, 0xC3, 0x9F, 0xBC, 0xC3, 0x5C, 0x14, 0x7E, 0x59, 0x82, 0x28, 0xC8, 0x82, 0x2A,
        0x50, 0x3F, 0xB3, 0x20, 0xEE, 0xC0, 0xA9, 0x43, 0x2B, 0xA5, 0x2F, 0xF0, 0xDA, 0xA4, 0x5F,
        0x7B, 0x07, 0x7E, 0x15, 0x01, 0x2E, 0xE3, 0x71, 0x3B, 0xA3, 0x1A, 0xFB, 0x4D, 0x60, 0xBB,
        0xAA, 0x00, 0xC1, 0x18, 0xC8, 0xBB, 0xAD, 0x00, 0xC1, 0x8E, 0xD0, 0x9C, 0x7C, 0x9B, 0x33,
        0x28, 0x4E, 0x83, 0xB1, 0xFF, 0x74, 0x0B, 0xB3, 0xC9, 0xF9, 0x3B, 0x70, 0x98, 0xAC, 0xE1,
        0x69, 0x9C, 0xB3, 0xBF, 0xE0, 0xAD, 0x07, 0xD3, 0xC3, 0xAF, 0x6A, 0xC8, 0xD2, 0xED, 0x13,
        0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];
    BYTES.to_vec()
}

fn stamped(text: &str, x: f32, y: f32) -> TextContent {
    TextContent::new(
        text,
        Rect::new(x, y, 100.0, 12.0),
        FontSpec::new("Helvetica", 12.0),
        TextStyle::default(),
    )
}

fn save(editor: &mut DocumentEditor) -> Vec<u8> {
    let options = SaveOptions {
        compress: false,
        ..SaveOptions::full_rewrite()
    };
    editor.save_to_bytes_with_options(options).unwrap()
}

/// Control. The same stamp on a page whose content stream leaves the CTM alone
/// lands where it was asked to, which is what isolates the failure below to the
/// inherited transform rather than to this harness or to the assertion.
///
/// This one passes on v0.3.77.
#[test]
fn overlay_text_lands_correctly_without_an_inherited_transform() {
    let upright = "BT /F1 12 Tf 1 0 0 1 72 72 Tm (original) Tj ET\n";
    let mut editor = DocumentEditor::open_from_bytes(page(upright)).unwrap();
    editor
        .edit_page(0, |page| {
            page.add_text(stamped("stamped", 72.0, 700.0));
            Ok(())
        })
        .unwrap();
    let saved = save(&mut editor);

    let document = PdfDocument::open_from_bytes(saved).unwrap();
    let chars = document.extract_page_text(0).unwrap().chars;
    let stamp = chars.iter().find(|char| char.char == 's').unwrap();

    assert!((stamp.bbox.y - 700.0).abs() < 12.0, "y={}", stamp.bbox.y);
}

/// `add_structure_element_impl` emits `EndMarkedContent` without first closing
/// the text object, so the overlay serialises `BT ... Tj EMC` with no `ET`.
///
/// The artifact branch of `add_text_content` already performs exactly this
/// guard, noting that "BDC must be outside BT/ET".
#[test]
fn overlay_text_closes_its_text_object() {
    let mut editor = DocumentEditor::open_from_bytes(page(FLIPPED)).unwrap();
    editor
        .edit_page(0, |page| {
            page.add_text(stamped("stamped", 72.0, 700.0));
            Ok(())
        })
        .unwrap();

    let saved = String::from_utf8_lossy(&save(&mut editor)).into_owned();

    assert_eq!(
        saved.matches("BT").count(),
        saved.matches("ET").count(),
        "unbalanced text object in:\n{saved}"
    );
}

/// The overlay stream is appended to `/Contents` with nothing wrapping the
/// original content, so it begins in whatever space the original stream left
/// behind rather than in page space.
///
/// Here that is a y-flip, so the stamp lands near the opposite edge and
/// mirrored. Same root cause as #1015, which reports it for the redaction
/// overlay rather than for `overlay_additions`.
#[test]
fn overlay_text_starts_in_page_space() {
    let mut editor = DocumentEditor::open_from_bytes(page(FLIPPED)).unwrap();
    editor
        .edit_page(0, |page| {
            page.add_text(stamped("stamped", 72.0, 700.0));
            Ok(())
        })
        .unwrap();
    let saved = save(&mut editor);

    let document = PdfDocument::open_from_bytes(saved).unwrap();
    let chars = document.extract_page_text(0).unwrap().chars;
    let stamp = chars
        .iter()
        .find(|char| char.char == 's')
        .expect("the stamped text is not on the page at all");

    assert!(
        (stamp.bbox.y - 700.0).abs() < 12.0,
        "stamp asked for y=700 landed at y={}, {} points away",
        stamp.bbox.y,
        (stamp.bbox.y - 700.0).abs()
    );
}

/// `generate_content_stream` returns the images the content stream referenced,
/// but the `overlay_additions` write site discards them --
/// `if let Ok((content_bytes, _pending))` -- and registers only fonts.
///
/// So the stream says `/Im1 Do` while the XObject is neither written nor named
/// in `/Resources/XObject`, and readers report `XObject 'Im1' is unknown`. The
/// full-rewrite path at the `modified_content` site handles this correctly and
/// is the model.
#[test]
fn overlay_image_is_registered_as_a_page_resource() {
    let mut editor = DocumentEditor::open_from_bytes(page(FLIPPED)).unwrap();
    editor
        .add_image_bytes_to_page(0, &png(), 72.0, 700.0, 64.0, 64.0)
        .unwrap();
    let saved = save(&mut editor);

    let document = PdfDocument::open_from_bytes(saved).unwrap();

    assert_eq!(
        document.extract_images(0).unwrap().len(),
        1,
        "the drawn image reached no page resource dictionary"
    );
}
