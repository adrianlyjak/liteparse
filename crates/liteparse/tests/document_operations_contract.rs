use liteparse::types::PdfInput;
use liteparse::{DocumentOperations, LiteParse, LiteParseConfig, OpenDocument};
use serial_test::serial;

const ACROFORM_PDF: &str = "../../integration_tests_data/filled_acroform.pdf";

fn parser() -> LiteParse {
    LiteParse::new(LiteParseConfig {
        ocr_enabled: false,
        quiet: true,
        ..Default::default()
    })
}

fn assert_document_operations<T, Input>()
where
    T: DocumentOperations<Input = Input>,
{
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

    let one_shot = parser
        .parse_pages_input(input.clone(), [3, 1, 3])
        .await
        .unwrap();
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
            .parse_pages_input(PdfInput::Path(ACROFORM_PDF.into()), &pages)
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

    let one_shot = parser
        .parse_pages_input(input.clone(), [3, 1, 3])
        .await
        .unwrap();
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
