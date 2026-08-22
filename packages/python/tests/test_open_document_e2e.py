"""Retained PDF Python API coverage."""

from pathlib import Path

import pytest

from liteparse import LiteParse, ParseError


@pytest.fixture
def sample_pdf() -> Path:
    return Path(__file__).resolve().parents[3] / "integration_tests_data" / "sample.pdf"


def test_open_document_parse_matches_one_shot(
    parser: LiteParse, sample_pdf: Path
) -> None:
    expected = parser.parse(sample_pdf)

    with parser.open_document(sample_pdf) as document:
        assert document.page_count == expected.total_pages
        assert document.parse().text == expected.text


def test_context_manager_closes_document(parser: LiteParse, sample_pdf: Path) -> None:
    with parser.open_document(sample_pdf) as document:
        document.parse()

    with pytest.raises(ParseError, match="document is closed"):
        document.parse()
