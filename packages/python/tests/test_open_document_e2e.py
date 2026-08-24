"""Retained PDF Python API coverage."""

from pathlib import Path

import pytest

from liteparse import LiteParse, OpenDocument, ParseError
from liteparse.parser import _DOCUMENT_OPERATION_NAMES


@pytest.fixture
def sample_pdf() -> Path:
    return Path(__file__).resolve().parents[3] / "integration_tests_data" / "sample.pdf"


@pytest.fixture
def three_page_pdf() -> Path:
    return (
        Path(__file__).resolve().parents[3]
        / "integration_tests_data"
        / "filled_acroform.pdf"
    )


@pytest.fixture
def receipt_image() -> Path:
    return Path(__file__).resolve().parents[3] / "integration_tests_data" / "receipt.png"


@pytest.fixture
def page_parser() -> LiteParse:
    return LiteParse(ocr_enabled=False)


def test_open_document_parse_matches_one_shot(
    parser: LiteParse, sample_pdf: Path
) -> None:
    expected = parser.parse(sample_pdf)

    with parser.open_document(sample_pdf) as document:
        assert document.page_count == expected.total_pages
        assert document.parse().text == expected.text


def test_document_operation_names_match_both_public_classes() -> None:
    assert _DOCUMENT_OPERATION_NAMES == {"parse", "parse_pages"}
    assert _DOCUMENT_OPERATION_NAMES <= set(vars(LiteParse))
    assert _DOCUMENT_OPERATION_NAMES <= set(vars(OpenDocument))


def test_selected_page_parse_matches_one_shot(
    page_parser: LiteParse, three_page_pdf: Path
) -> None:
    one_shot = page_parser.parse_pages(three_page_pdf, [3, 1, 3])

    with page_parser.open_document(three_page_pdf) as document:
        retained = document.parse_pages([3, 1, 3])

    assert one_shot.total_pages == retained.total_pages
    assert one_shot.text == retained.text
    assert [(page.page_num, page.text) for page in one_shot.pages] == [
        (page.page_num, page.text) for page in retained.pages
    ]


def test_open_document_converts_supported_input(
    page_parser: LiteParse, receipt_image: Path
) -> None:
    with page_parser.open_document(receipt_image) as document:
        assert document.page_count == 1
        before = document.parse()
        document.reopen()
        after = document.parse()

    assert after.total_pages == 1
    assert after.text == before.text


def test_context_manager_closes_document(parser: LiteParse, sample_pdf: Path) -> None:
    with parser.open_document(sample_pdf) as document:
        document.parse()

    with pytest.raises(ParseError, match="document is closed"):
        document.parse()
    with pytest.raises(ParseError, match="document is closed"):
        document.reopen()


def test_parse_pages_sorts_and_deduplicates_in_source_order(
    page_parser: LiteParse, three_page_pdf: Path
) -> None:
    with page_parser.open_document(three_page_pdf) as document:
        parsed = document.parse_pages([3, 1, 3])

    assert parsed.total_pages == 3
    assert [page.page_num for page in parsed.pages] == [1, 3]


@pytest.mark.parametrize(
    ("pages", "message"),
    [
        ([], "page selection cannot be empty"),
        ([0], r"page 0 out of range \(document has 3 pages\)"),
        ([1, 4], r"page 4 out of range \(document has 3 pages\)"),
    ],
)
def test_parse_pages_validates_the_entire_selection_before_parsing(
    page_parser: LiteParse,
    three_page_pdf: Path,
    pages: list[int],
    message: str,
) -> None:
    with pytest.raises(ParseError, match=message):
        page_parser.parse_pages(three_page_pdf, pages)

    with page_parser.open_document(three_page_pdf) as document:
        with pytest.raises(ParseError, match=message):
            document.parse_pages(pages)

        assert [page.page_num for page in document.parse_pages([2]).pages] == [2]


def test_parse_pages_applies_max_pages_after_normalization(
    three_page_pdf: Path,
) -> None:
    parser = LiteParse(ocr_enabled=False, max_pages=1)
    one_shot = parser.parse_pages(three_page_pdf, [3, 1, 3])

    with parser.open_document(three_page_pdf) as document:
        retained = document.parse_pages([3, 1, 3])

    assert one_shot.total_pages == retained.total_pages == 3
    assert [page.page_num for page in one_shot.pages] == [1]
    assert one_shot.text == retained.text


def test_parse_pages_ignores_configured_target_pages(three_page_pdf: Path) -> None:
    parser = LiteParse(ocr_enabled=False, target_pages="not-a-page-range")
    one_shot = parser.parse_pages(three_page_pdf, [2])

    with parser.open_document(three_page_pdf) as document:
        retained = document.parse_pages([2])

    assert [page.page_num for page in one_shot.pages] == [2]
    assert one_shot.text == retained.text


@pytest.mark.parametrize("pages", [[], [0], [4]])
def test_parse_pages_reports_closed_before_selection_errors(
    page_parser: LiteParse, three_page_pdf: Path, pages: list[int]
) -> None:
    document = page_parser.open_document(three_page_pdf)
    document.close()

    with pytest.raises(ParseError, match="document is closed"):
        document.parse_pages(pages)
