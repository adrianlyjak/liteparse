use liteparse::types::PdfInput;
use liteparse::{DocumentOperations, LiteParse, LiteParseConfig};
use serial_test::serial;

const ACROFORM_PDF: &str = "../../integration_tests_data/filled_acroform.pdf";

fn assert_document_operations<T, Input>()
where
    T: DocumentOperations<Input = Input>,
{
}

#[tokio::test]
#[serial]
async fn one_shot_parser_implements_selected_page_operations() {
    assert_document_operations::<LiteParse, PdfInput>();
    let parser = LiteParse::new(LiteParseConfig {
        ocr_enabled: false,
        quiet: true,
        ..Default::default()
    });

    let parsed =
        DocumentOperations::parse_pages(&parser, PdfInput::Path(ACROFORM_PDF.into()), [3, 1, 3])
            .await
            .unwrap();

    assert_eq!(parsed.total_pages, 3);
    assert_eq!(
        parsed
            .pages
            .iter()
            .map(|page| page.page_number)
            .collect::<Vec<_>>(),
        vec![1, 3]
    );
}
