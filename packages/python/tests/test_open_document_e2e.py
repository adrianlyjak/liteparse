"""Retained PDF and raw raster Python API coverage."""

from pathlib import Path

import pytest

from liteparse import (
    LiteParse,
    OpenDocument,
    PageRasterOptions,
    ParseError,
)
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
    assert _DOCUMENT_OPERATION_NAMES == {
        "parse",
        "parse_pages",
        "raster_page",
        "screenshot_pages",
    }
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


@pytest.mark.parametrize("pages", [[], [0], [4]])
def test_parse_pages_reports_closed_before_selection_errors(
    page_parser: LiteParse, three_page_pdf: Path, pages: list[int]
) -> None:
    document = page_parser.open_document(three_page_pdf)
    document.close()

    with pytest.raises(ParseError, match="document is closed"):
        document.parse_pages(pages)


def test_screenshot_pages_matches_one_shot_and_preserves_order(
    three_page_pdf: Path,
) -> None:
    parser = LiteParse(
        ocr_enabled=False,
        dpi=96,
        render_form_fields=True,
    )
    expected = parser.screenshot_pages(three_page_pdf, [3, 1, 3])

    with parser.open_document(three_page_pdf) as document:
        actual = document.screenshot_pages([3, 1, 3])

    assert [page.page_num for page in actual] == [3, 1, 3]
    assert [page.image_bytes for page in actual] == [
        page.image_bytes for page in expected
    ]


@pytest.mark.parametrize(
    ("pages", "message"),
    [
        ([], "page selection cannot be empty"),
        ([0], r"page 0 out of range \(document has 3 pages\)"),
        ([1, 4], r"page 4 out of range \(document has 3 pages\)"),
    ],
)
def test_screenshot_pages_validates_selection(
    page_parser: LiteParse,
    three_page_pdf: Path,
    pages: list[int],
    message: str,
) -> None:
    with pytest.raises(ParseError, match=message) as one_shot:
        page_parser.screenshot_pages(three_page_pdf, pages)

    with page_parser.open_document(three_page_pdf) as document:
        with pytest.raises(ParseError, match=message) as retained:
            document.screenshot_pages(pages)

    assert str(one_shot.value) == str(retained.value)


def test_screenshot_pages_rejects_closed_document(
    page_parser: LiteParse, three_page_pdf: Path
) -> None:
    document = page_parser.open_document(three_page_pdf)
    document.close()

    with pytest.raises(ParseError, match="document is closed"):
        document.screenshot_pages([1])


@pytest.mark.parametrize("as_bytes", [False, True], ids=["path", "bytes"])
def test_raster_page_matches_one_shot_for_path_and_bytes(
    parser: LiteParse, sample_pdf: Path, as_bytes: bool
) -> None:
    source: Path | bytes = sample_pdf.read_bytes() if as_bytes else sample_pdf
    options = PageRasterOptions(
        dpi=36,
        pixel_format="rgbx8",
        render_form_fields=True,
    )
    expected = parser.raster_page(source, 1, options)

    with parser.open_document(source) as document:
        actual = document.raster_page(1, options)

    assert actual.page_num == expected.page_num
    assert actual.width == expected.width
    assert actual.height == expected.height
    assert actual.stride == expected.stride
    assert actual.pixel_format == expected.pixel_format
    assert actual.pixels == expected.pixels


def test_open_document_raster_formats(parser: LiteParse, sample_pdf: Path) -> None:
    with parser.open_document(sample_pdf) as document:
        rgb = document.raster_page(1, PageRasterOptions(dpi=36, pixel_format="rgb8"))
        rgbx = document.raster_page(1, PageRasterOptions(dpi=36, pixel_format="rgbx8"))

    assert (rgb.width, rgb.height) == (rgbx.width, rgbx.height)
    assert rgb.stride == rgb.width * 3
    assert rgbx.stride == rgbx.width * 4
    assert len(rgb.pixels) == rgb.stride * rgb.height
    assert len(rgbx.pixels) == rgbx.stride * rgbx.height


@pytest.mark.parametrize(
    ("page_num", "options", "message"),
    [
        (0, PageRasterOptions(), r"page 0 out of range \(document has 1 pages\)"),
        (2, PageRasterOptions(), r"page 2 out of range \(document has 1 pages\)"),
        (
            1,
            PageRasterOptions(dpi=0),
            "raster dpi must be a positive finite number",
        ),
        (
            1,
            PageRasterOptions(dpi=float("nan")),
            "raster dpi must be a positive finite number",
        ),
        (
            1,
            PageRasterOptions(dpi=100_000),
            "raster exceeds the 256 MiB pixel buffer limit",
        ),
    ],
)
def test_raster_page_validation_matches_document_modes(
    parser: LiteParse,
    sample_pdf: Path,
    page_num: int,
    options: PageRasterOptions,
    message: str,
) -> None:
    with pytest.raises(ParseError, match=message) as one_shot:
        parser.raster_page(sample_pdf, page_num, options)

    with parser.open_document(sample_pdf) as document:
        with pytest.raises(ParseError, match=message) as retained:
            document.raster_page(page_num, options)

    assert str(one_shot.value) == str(retained.value)


def test_closed_document_rejects_work(parser: LiteParse, sample_pdf: Path) -> None:
    document = parser.open_document(sample_pdf)
    document.close()
    document.close()

    with pytest.raises(ParseError, match="document is closed"):
        document.raster_page(1)
