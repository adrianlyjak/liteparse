use liteparse::types::PdfInput;
use liteparse::{
    DocumentOperations, LiteParse, LiteParseConfig, OpenDocument, PageRaster, PageRasterOptions,
    RasterPixelFormat,
};
use serial_test::serial;

const ACROFORM_PDF: &str = "../../integration_tests_data/filled_acroform.pdf";

fn parser() -> LiteParse {
    LiteParse::new(LiteParseConfig {
        ocr_enabled: false,
        quiet: true,
        ..Default::default()
    })
}

fn screenshot_parser() -> LiteParse {
    LiteParse::new(LiteParseConfig {
        ocr_enabled: false,
        quiet: true,
        dpi: 96.0,
        render_form_fields: true,
        ..Default::default()
    })
}

fn assert_document_operations<T, Input>()
where
    T: DocumentOperations<Input = Input>,
{
}

fn assert_same_raster(one_shot: &PageRaster, retained: &PageRaster) {
    assert_eq!(one_shot.page_num, retained.page_num);
    assert_eq!(one_shot.width, retained.width);
    assert_eq!(one_shot.height, retained.height);
    assert_eq!(one_shot.stride, retained.stride);
    assert_eq!(one_shot.pixel_format, retained.pixel_format);
    assert_eq!(one_shot.pixels, retained.pixels);
}

#[test]
fn document_operations_uses_the_document_source_as_an_associated_type() {
    assert_document_operations::<LiteParse, PdfInput>();
    assert_document_operations::<OpenDocument, ()>();
}

#[tokio::test]
#[serial]
async fn selected_page_parse_matches_between_one_shot_and_retained_documents() {
    let parser = parser();
    let input = PdfInput::Path(ACROFORM_PDF.into());

    let one_shot = parser.parse_pages(input.clone(), [3, 1, 3]).await.unwrap();
    let document = parser.open_document(input).await.unwrap();
    let retained = document.parse_pages([3, 1, 3]).await.unwrap();

    assert_eq!(one_shot.total_pages, retained.total_pages);
    assert_eq!(one_shot.text, retained.text);
    assert_eq!(
        one_shot
            .pages
            .iter()
            .map(|page| (page.page_number, &page.text))
            .collect::<Vec<_>>(),
        retained
            .pages
            .iter()
            .map(|page| (page.page_number, &page.text))
            .collect::<Vec<_>>()
    );
}

#[tokio::test]
#[serial]
async fn trait_calls_preserve_natural_source_arity() {
    let parser = parser();
    let input = PdfInput::Path(ACROFORM_PDF.into());
    let document = parser.open_document(input.clone()).await.unwrap();

    let one_shot = DocumentOperations::parse_pages(&parser, input, [2])
        .await
        .unwrap();
    let retained = DocumentOperations::parse_pages(&document, (), [2])
        .await
        .unwrap();

    assert_eq!(one_shot.text, retained.text);
    assert_eq!(one_shot.pages[0].page_number, 2);

    let one_shot = DocumentOperations::screenshot_pages(
        &parser,
        PdfInput::Path(ACROFORM_PDF.into()),
        [3, 1, 3],
    )
    .await
    .unwrap();
    let retained = DocumentOperations::screenshot_pages(&document, (), [3, 1, 3])
        .await
        .unwrap();

    assert_eq!(
        one_shot
            .iter()
            .map(|page| page.page_num)
            .collect::<Vec<_>>(),
        vec![3, 1, 3]
    );
    assert_eq!(
        one_shot
            .iter()
            .map(|page| page.image_bytes.as_slice())
            .collect::<Vec<_>>(),
        retained
            .iter()
            .map(|page| page.image_bytes.as_slice())
            .collect::<Vec<_>>()
    );

    let options = PageRasterOptions {
        dpi: 96.0,
        pixel_format: RasterPixelFormat::Rgbx8,
        render_form_fields: true,
    };
    let one_shot =
        DocumentOperations::raster_page(&parser, PdfInput::Path(ACROFORM_PDF.into()), 2, options)
            .await
            .unwrap();
    let retained = DocumentOperations::raster_page(&document, (), 2, options)
        .await
        .unwrap();

    assert_same_raster(&one_shot, &retained);
}

#[tokio::test]
#[serial]
async fn raster_page_matches_between_one_shot_path_and_retained_document() {
    let parser = screenshot_parser();
    let input = PdfInput::Path(ACROFORM_PDF.into());
    let options = PageRasterOptions {
        dpi: 72.0,
        pixel_format: RasterPixelFormat::Rgb8,
        render_form_fields: true,
    };

    let one_shot = parser.raster_page(input.clone(), 1, options).await.unwrap();
    let retained = parser
        .open_document(input)
        .await
        .unwrap()
        .raster_page(1, options)
        .unwrap();

    assert_same_raster(&one_shot, &retained);
}

#[tokio::test]
#[serial]
async fn raster_page_accepts_pdf_bytes_and_matches_retained_document() {
    let parser = screenshot_parser();
    let bytes = std::fs::read(ACROFORM_PDF).unwrap();
    let input = PdfInput::Bytes(bytes);
    let options = PageRasterOptions {
        dpi: 36.0,
        pixel_format: RasterPixelFormat::Rgbx8,
        render_form_fields: true,
    };

    let one_shot = parser.raster_page(input.clone(), 3, options).await.unwrap();
    let retained = parser
        .open_document(input)
        .await
        .unwrap()
        .raster_page(3, options)
        .unwrap();

    assert_same_raster(&one_shot, &retained);
}

#[tokio::test]
#[serial]
async fn raster_page_validation_matches_between_document_modes() {
    let parser = screenshot_parser();
    let input = PdfInput::Path(ACROFORM_PDF.into());
    let document = parser.open_document(input.clone()).await.unwrap();

    for (page_num, options, expected) in [
        (
            0,
            PageRasterOptions::default(),
            "page 0 out of range (document has 3 pages)",
        ),
        (
            4,
            PageRasterOptions::default(),
            "page 4 out of range (document has 3 pages)",
        ),
        (
            1,
            PageRasterOptions {
                dpi: 0.0,
                ..Default::default()
            },
            "invalid config: raster dpi must be a positive finite number",
        ),
        (
            1,
            PageRasterOptions {
                dpi: f32::NAN,
                ..Default::default()
            },
            "invalid config: raster dpi must be a positive finite number",
        ),
    ] {
        let one_shot = parser
            .raster_page(input.clone(), page_num, options)
            .await
            .expect_err("invalid one-shot raster arguments should fail");
        let retained = document
            .raster_page(page_num, options)
            .expect_err("invalid retained raster arguments should fail");

        assert_eq!(one_shot.to_string(), expected);
        assert_eq!(one_shot.to_string(), retained.to_string());
    }
}

#[tokio::test]
#[serial]
async fn screenshot_pages_matches_between_one_shot_and_retained_documents() {
    let parser = screenshot_parser();
    let input = PdfInput::Path(ACROFORM_PDF.into());
    let selection = [3, 1, 3];

    let one_shot = parser
        .screenshot_pages(input.clone(), selection)
        .await
        .unwrap();
    let document = parser.open_document(input).await.unwrap();
    let retained = document.screenshot_pages(selection).unwrap();

    assert_eq!(
        one_shot
            .iter()
            .map(|page| page.page_num)
            .collect::<Vec<_>>(),
        vec![3, 1, 3]
    );
    assert_eq!(one_shot.len(), retained.len());
    for (one_shot, retained) in one_shot.iter().zip(&retained) {
        assert_eq!(one_shot.page_num, retained.page_num);
        assert_eq!(one_shot.width, retained.width);
        assert_eq!(one_shot.height, retained.height);
        assert_eq!(one_shot.image_bytes, retained.image_bytes);
    }
}

#[tokio::test]
#[serial]
async fn screenshot_page_validation_matches_between_document_modes() {
    let parser = screenshot_parser();
    let input = PdfInput::Path(ACROFORM_PDF.into());
    let document = parser.open_document(input.clone()).await.unwrap();

    for (pages, expected) in [
        (vec![], "page selection cannot be empty"),
        (vec![0], "page 0 out of range (document has 3 pages)"),
        (vec![1, 4], "page 4 out of range (document has 3 pages)"),
    ] {
        let one_shot = parser
            .screenshot_pages(input.clone(), &pages)
            .await
            .expect_err("an invalid one-shot selection should fail");
        let retained = document
            .screenshot_pages(&pages)
            .expect_err("an invalid retained selection should fail");

        assert_eq!(one_shot.to_string(), expected);
        assert_eq!(one_shot.to_string(), retained.to_string());
    }
}

#[tokio::test]
#[serial]
async fn selected_page_validation_matches_between_document_modes() {
    let parser = parser();

    for (pages, expected) in [
        (vec![], "page selection cannot be empty"),
        (vec![0], "page 0 out of range (document has 3 pages)"),
        (vec![1, 4], "page 4 out of range (document has 3 pages)"),
    ] {
        let one_shot = match parser
            .parse_pages(PdfInput::Path(ACROFORM_PDF.into()), &pages)
            .await
        {
            Ok(_) => panic!("an invalid one-shot selection should fail"),
            Err(error) => error,
        };
        let document = parser
            .open_document(PdfInput::Path(ACROFORM_PDF.into()))
            .await
            .unwrap();
        let retained = match document.parse_pages(&pages).await {
            Ok(_) => panic!("an invalid retained selection should fail"),
            Err(error) => error,
        };

        assert_eq!(one_shot.to_string(), expected);
        assert_eq!(one_shot.to_string(), retained.to_string());
    }
}

#[tokio::test]
#[serial]
async fn selected_page_parse_normalizes_before_max_pages_and_ignores_target_pages() {
    let parser = LiteParse::new(LiteParseConfig {
        ocr_enabled: false,
        quiet: true,
        max_pages: 1,
        target_pages: Some("not-a-page-range".into()),
        ..Default::default()
    });
    let input = PdfInput::Path(ACROFORM_PDF.into());

    let one_shot = parser.parse_pages(input.clone(), [3, 1, 3]).await.unwrap();
    let retained = parser
        .open_document(input)
        .await
        .unwrap()
        .parse_pages([3, 1, 3])
        .await
        .unwrap();

    assert_eq!(one_shot.total_pages, 3);
    assert_eq!(one_shot.pages.len(), 1);
    assert_eq!(one_shot.pages[0].page_number, 1);
    assert_eq!(one_shot.text, retained.text);
}
