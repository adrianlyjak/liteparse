"""Retained PDF and raw-raster Python API coverage."""

from pathlib import Path

import pytest

from liteparse import LiteParse, PageRasterOptions, ParseError


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


def test_open_document_raster_formats(parser: LiteParse, sample_pdf: Path) -> None:
    with parser.open_document(sample_pdf) as document:
        rgb = document.raster_page(1, PageRasterOptions(dpi=36, pixel_format="rgb8"))
        rgbx = document.raster_page(1, PageRasterOptions(dpi=36, pixel_format="rgbx8"))

    assert (rgb.width, rgb.height) == (rgbx.width, rgbx.height)
    assert rgb.stride == rgb.width * 3
    assert rgbx.stride == rgbx.width * 4
    assert len(rgb.pixels) == rgb.stride * rgb.height
    assert len(rgbx.pixels) == rgbx.stride * rgbx.height


def test_closed_document_rejects_work(parser: LiteParse, sample_pdf: Path) -> None:
    document = parser.open_document(sample_pdf)
    document.close()
    document.close()

    with pytest.raises(ParseError, match="document is closed"):
        document.raster_page(1)
