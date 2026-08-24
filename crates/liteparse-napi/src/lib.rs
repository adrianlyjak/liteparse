use napi::bindgen_prelude::*;
use napi_derive::napi;

mod types;

use types::{
    JsLiteParseConfig, JsPageComplexityStats, JsPageInput, JsPageRaster, JsPageRasterOptions,
    JsParseBatch, JsParseResult, JsScreenshotResult, JsTextItem,
};

fn page_number_from_js(page_number: f64) -> Result<u32> {
    if page_number.is_finite()
        && page_number.fract() == 0.0
        && page_number >= 0.0
        && page_number <= u32::MAX as f64
    {
        Ok(page_number as u32)
    } else {
        Err(Error::from_reason(format!(
            "page number must be a finite integer representable as u32: {page_number}"
        )))
    }
}

fn page_numbers_from_js(page_numbers: Vec<f64>) -> Result<Vec<u32>> {
    page_numbers.into_iter().map(page_number_from_js).collect()
}

/// Main LiteParse parser class.
#[napi]
pub struct LiteParse {
    inner: liteparse::parser::LiteParse,
    config: liteparse::config::LiteParseConfig,
}

#[napi]
impl LiteParse {
    /// Create a new LiteParse instance with optional configuration.
    /// Any fields not provided will use defaults.
    #[napi(constructor)]
    pub fn new(config: Option<JsLiteParseConfig>) -> Self {
        let rust_config = config.map(|c| c.into_rust()).unwrap_or_default();
        let inner = liteparse::parser::LiteParse::new(rust_config.clone());
        Self {
            inner,
            config: rust_config,
        }
    }

    /// Parse a document. Accepts a file path (string) or raw PDF bytes (Buffer).
    #[napi]
    pub async fn parse(&self, input: Either<String, Buffer>) -> Result<JsParseResult> {
        use liteparse::types::PdfInput;

        let pdf_input = match input {
            Either::A(path) => PdfInput::Path(path),
            Either::B(buf) => PdfInput::Bytes(buf.to_vec()),
        };

        let result = self
            .inner
            .parse_input(pdf_input)
            .await
            .map_err(|e| Error::from_reason(e.to_string()))?;

        Ok(JsParseResult::from_rust(&result, &self.config))
    }

    /// Parse an explicit set of 1-based pages from a document source.
    #[napi]
    pub async fn parse_source_pages(
        &self,
        input: Either<String, Buffer>,
        page_numbers: Vec<f64>,
    ) -> Result<JsParseResult> {
        use liteparse::types::PdfInput;

        let pdf_input = match input {
            Either::A(path) => PdfInput::Path(path),
            Either::B(buf) => PdfInput::Bytes(buf.to_vec()),
        };
        let page_numbers = page_numbers_from_js(page_numbers)?;
        let result = self
            .inner
            .parse_pages_input(pdf_input, page_numbers)
            .await
            .map_err(|error| Error::from_reason(error.to_string()))?;
        Ok(JsParseResult::from_rust(&result, &self.config))
    }

    /// Open a document for repeated page operations.
    #[napi]
    pub async fn open_document(&self, input: Either<String, Buffer>) -> Result<OpenDocument> {
        let input = match input {
            Either::A(path) => liteparse::types::PdfInput::Path(path),
            Either::B(buf) => liteparse::types::PdfInput::Bytes(buf.to_vec()),
        };
        let document = self
            .inner
            .open_document(input)
            .await
            .map_err(|error| Error::from_reason(error.to_string()))?;
        Ok(OpenDocument {
            inner: std::sync::Arc::new(document),
            config: self.config.clone(),
        })
    }

    /// Open a document for bounded-memory batch parsing. Internal plumbing
    /// for the JS wrapper's `parseBatches()` — prefer that; it also closes
    /// the session for you.
    ///
    /// Converts a non-PDF source once, then yields `batchSize` pages at a time
    /// via `nextBatch()` (default 25). Cross-page passes (repeated
    /// header/footer removal, image deduplication) see only the pages in their
    /// own batch, so output can differ from a whole-document `parse()`.
    #[napi]
    pub async fn open_batch_session(
        &self,
        input: Either<String, Buffer>,
        batch_size: Option<u32>,
    ) -> Result<ParseSession> {
        use liteparse::types::PdfInput;

        let pdf_input = match input {
            Either::A(path) => PdfInput::Path(path),
            Either::B(buf) => PdfInput::Bytes(buf.to_vec()),
        };
        let batch_size = batch_size
            .map(|v| v as usize)
            .unwrap_or(liteparse::DEFAULT_PAGE_BATCH_SIZE);

        let session = self
            .inner
            .open_batch_session(pdf_input, batch_size)
            .await
            .map_err(|e| Error::from_reason(e.to_string()))?;

        Ok(ParseSession {
            total_pages: session.total_pages(),
            inner: std::sync::Arc::new(tokio::sync::Mutex::new(Some(session))),
            config: self.config.clone(),
        })
    }

    /// Parse from pre-extracted pages, skipping PDFium text extraction.
    ///
    /// The caller supplies pages already populated with text items in viewport
    /// space (top-left origin, 72 DPI). Runs only grid projection + the
    /// configured output formatter, so it never loads PDFium. Use when an
    /// external extractor owns text extraction (e.g. to keep its own
    /// font-recovery pipeline).
    #[napi]
    pub fn parse_pages(&self, pages: Vec<JsPageInput>) -> Result<JsParseResult> {
        let rust_pages: Vec<_> = pages.iter().map(JsPageInput::to_rust).collect();
        let result = self.inner.parse_from_pages(rust_pages, Vec::new());
        Ok(JsParseResult::from_rust(&result, &self.config))
    }

    /// Determine per-page complexity. Returns one entry per parsed page with
    /// signals (text coverage, images, garbled text, vector area) and a
    /// `needsOcr` verdict — a cheap pre-OCR check to decide whether a document
    /// needs advanced parsing. Accepts a file path (string) or raw PDF bytes.
    #[napi]
    pub async fn is_complex(
        &self,
        input: Either<String, Buffer>,
    ) -> Result<Vec<JsPageComplexityStats>> {
        use liteparse::types::PdfInput;

        let pdf_input = match input {
            Either::A(path) => PdfInput::Path(path),
            Either::B(buf) => PdfInput::Bytes(buf.to_vec()),
        };

        let stats = self
            .inner
            .is_complex(pdf_input)
            .await
            .map_err(|e| Error::from_reason(e.to_string()))?;

        Ok(stats.iter().map(JsPageComplexityStats::from_rust).collect())
    }

    /// Take screenshots of document pages. Returns PNG image buffers.
    ///
    /// Non-PDF files are automatically converted to PDF before rendering when
    /// LibreOffice/ImageMagick are available.
    #[napi]
    pub async fn screenshot(
        &self,
        input: Either<String, Buffer>,
        page_numbers: Option<Vec<u32>>,
    ) -> Result<Vec<JsScreenshotResult>> {
        use liteparse::types::PdfInput;

        let pdf_input = match input {
            Either::A(path) => PdfInput::Path(path),
            Either::B(buf) => PdfInput::Bytes(buf.to_vec()),
        };

        let results = self
            .inner
            .screenshot_input(pdf_input, page_numbers)
            .await
            .map_err(|e| Error::from_reason(e.to_string()))?;

        Ok(results.into_iter().map(JsScreenshotResult::from).collect())
    }

    /// Render explicit 1-based source pages as PNG screenshots.
    #[napi]
    pub async fn screenshot_pages(
        &self,
        input: Either<String, Buffer>,
        page_numbers: Vec<f64>,
    ) -> Result<Vec<JsScreenshotResult>> {
        use liteparse::types::PdfInput;

        let pdf_input = match input {
            Either::A(path) => PdfInput::Path(path),
            Either::B(buf) => PdfInput::Bytes(buf.to_vec()),
        };
        let page_numbers = page_numbers_from_js(page_numbers)?;
        let results = self
            .inner
            .screenshot_pages_input(pdf_input, page_numbers)
            .await
            .map_err(|error| Error::from_reason(error.to_string()))?;
        Ok(results.into_iter().map(JsScreenshotResult::from).collect())
    }

    /// Render one 1-based source page to an owned, unencoded pixel buffer.
    #[napi]
    pub async fn raster_page(
        &self,
        input: Either<String, Buffer>,
        page_num: f64,
        options: Option<JsPageRasterOptions>,
    ) -> Result<JsPageRaster> {
        use liteparse::types::PdfInput;

        let pdf_input = match input {
            Either::A(path) => PdfInput::Path(path),
            Either::B(buf) => PdfInput::Bytes(buf.to_vec()),
        };
        let page_num = page_number_from_js(page_num)?;
        let raster = self
            .inner
            .raster_page_input(pdf_input, page_num, options.unwrap_or_default().into_rust())
            .await
            .map_err(|error| Error::from_reason(error.to_string()))?;
        Ok(JsPageRaster::from_rust(raster))
    }

    /// Get the current configuration.
    #[napi(getter)]
    pub fn config(&self) -> JsLiteParseConfig {
        JsLiteParseConfig::from_rust(&self.config)
    }
}

/// A document normalized to PDF and kept open for repeated page operations.
#[napi]
pub struct OpenDocument {
    inner: std::sync::Arc<liteparse::OpenDocument>,
    config: liteparse::config::LiteParseConfig,
}

#[napi]
impl OpenDocument {
    /// Total pages in the source document.
    #[napi(getter)]
    pub fn page_count(&self) -> u32 {
        self.inner.page_count()
    }

    /// Parse the retained document.
    #[napi]
    pub async fn parse(&self) -> Result<JsParseResult> {
        let result = self
            .inner
            .parse()
            .await
            .map_err(|error| Error::from_reason(error.to_string()))?;
        Ok(JsParseResult::from_rust(&result, &self.config))
    }

    /// Parse an explicit set of 1-based source pages.
    #[napi]
    pub async fn parse_pages(&self, page_numbers: Vec<f64>) -> Result<JsParseResult> {
        let page_numbers = page_numbers_from_js(page_numbers)?;
        let result = self
            .inner
            .parse_pages(page_numbers)
            .await
            .map_err(|error| Error::from_reason(error.to_string()))?;
        Ok(JsParseResult::from_rust(&result, &self.config))
    }

    /// Render explicit 1-based source pages as PNG screenshots.
    #[napi(ts_return_type = "Promise<Array<JsScreenshotResult>>")]
    pub fn screenshot_pages(&self, page_numbers: Vec<f64>) -> AsyncTask<ScreenshotPagesTask> {
        AsyncTask::new(ScreenshotPagesTask {
            document: self.inner.clone(),
            page_numbers,
        })
    }

    /// Render one 1-based page to an owned, unencoded pixel buffer.
    #[napi(ts_return_type = "Promise<JsPageRaster>")]
    pub fn raster_page(
        &self,
        page_num: f64,
        options: Option<JsPageRasterOptions>,
    ) -> AsyncTask<RasterPageTask> {
        AsyncTask::new(RasterPageTask {
            document: self.inner.clone(),
            page_num,
            options: options.unwrap_or_default(),
        })
    }

    /// Reopen the PDFium document while retaining the normalized PDF.
    #[napi(ts_return_type = "Promise<void>")]
    pub fn reopen(&self) -> AsyncTask<DocumentTask> {
        AsyncTask::new(DocumentTask {
            document: self.inner.clone(),
            operation: DocumentOperation::Reopen,
        })
    }

    /// Release the retained document. Idempotent.
    #[napi(ts_return_type = "Promise<void>")]
    pub fn close(&self) -> AsyncTask<DocumentTask> {
        AsyncTask::new(DocumentTask {
            document: self.inner.clone(),
            operation: DocumentOperation::Close,
        })
    }
}

pub struct ScreenshotPagesTask {
    document: std::sync::Arc<liteparse::OpenDocument>,
    page_numbers: Vec<f64>,
}

#[napi]
impl Task for ScreenshotPagesTask {
    type Output = Vec<liteparse::ScreenshotResult>;
    type JsValue = Vec<JsScreenshotResult>;

    fn compute(&mut self) -> Result<Self::Output> {
        let page_numbers = page_numbers_from_js(std::mem::take(&mut self.page_numbers))?;
        self.document
            .screenshot_pages(page_numbers)
            .map_err(|error| Error::from_reason(error.to_string()))
    }

    fn resolve(&mut self, _env: Env, screenshots: Self::Output) -> Result<Self::JsValue> {
        Ok(screenshots
            .into_iter()
            .map(JsScreenshotResult::from)
            .collect())
    }
}

pub struct RasterPageTask {
    document: std::sync::Arc<liteparse::OpenDocument>,
    page_num: f64,
    options: JsPageRasterOptions,
}

#[napi]
impl Task for RasterPageTask {
    type Output = liteparse::PageRaster;
    type JsValue = JsPageRaster;

    fn compute(&mut self) -> Result<Self::Output> {
        let page_num = page_number_from_js(self.page_num)?;
        self.document
            .raster_page(page_num, self.options.clone().into_rust())
            .map_err(|error| Error::from_reason(error.to_string()))
    }

    fn resolve(&mut self, _env: Env, raster: Self::Output) -> Result<Self::JsValue> {
        Ok(JsPageRaster::from_rust(raster))
    }
}

enum DocumentOperation {
    Reopen,
    Close,
}

pub struct DocumentTask {
    document: std::sync::Arc<liteparse::OpenDocument>,
    operation: DocumentOperation,
}

#[napi]
impl Task for DocumentTask {
    type Output = ();
    type JsValue = ();

    fn compute(&mut self) -> Result<Self::Output> {
        match self.operation {
            DocumentOperation::Reopen => self
                .document
                .reopen()
                .map_err(|error| Error::from_reason(error.to_string())),
            DocumentOperation::Close => {
                self.document.close();
                Ok(())
            }
        }
    }

    fn resolve(&mut self, _env: Env, (): Self::Output) -> Result<Self::JsValue> {
        Ok(())
    }
}

/// A document opened once and parsed in bounded page batches. Internal
/// plumbing for the JS wrapper's `parseBatches()` — prefer that.
///
/// Created by `LiteParse.openBatchSession()`. The converted-PDF temporary
/// file for a non-PDF source lives as long as the session, so conversion is
/// paid once no matter how many batches are consumed. Call `close()` when
/// abandoning the session early — otherwise that temp file waits for GC.
#[napi]
pub struct ParseSession {
    /// The core session is `&mut` per batch, but napi hands out `&self`, so
    /// the mutation is serialized here. Concurrent `nextBatch()` calls queue
    /// rather than interleave, which also keeps batch order well-defined.
    /// `None` after `close()`.
    inner: std::sync::Arc<tokio::sync::Mutex<Option<liteparse::ParseSession>>>,
    config: liteparse::config::LiteParseConfig,
    total_pages: u32,
}

#[napi]
impl ParseSession {
    /// Total pages in the source document, before `maxPages` or batching.
    #[napi(getter)]
    pub fn total_pages(&self) -> u32 {
        self.total_pages
    }

    /// Parse and return the next batch, or `null` once every page within
    /// `maxPages` has been yielded. Rejects if the session is closed.
    #[napi]
    pub async fn next_batch(&self) -> Result<Option<JsParseBatch>> {
        let inner = self.inner.clone();
        let mut session = inner.lock().await;
        let batch = session
            .as_mut()
            .ok_or_else(|| Error::from_reason("session is closed"))?
            .next_batch()
            .await
            .map_err(|e| Error::from_reason(e.to_string()))?;

        Ok(batch.map(|batch| JsParseBatch {
            start_page: batch.start_page,
            end_page: batch.end_page,
            result: JsParseResult::from_rust(&batch.result, &self.config),
        }))
    }

    /// Release the session's resources now — most importantly the converted
    /// temporary PDF for a non-PDF source, which otherwise lives until the
    /// JS object is garbage collected. Idempotent; `nextBatch()` rejects
    /// afterwards.
    #[napi]
    pub async fn close(&self) -> Result<()> {
        let inner = self.inner.clone();
        let mut session = inner.lock().await;
        // Dropping the core session drops the conversion guard, which
        // removes the temp file.
        session.take();
        Ok(())
    }
}

/// Search text items for phrase matches, returning merged items with combined bounding boxes.
#[napi]
pub fn search_items(
    items: Vec<JsTextItem>,
    phrase: String,
    case_sensitive: Option<bool>,
) -> Vec<JsTextItem> {
    let rust_items: Vec<_> = items.iter().map(|i| i.to_rust()).collect();
    let options = liteparse::search::SearchOptions {
        phrase,
        case_sensitive: case_sensitive.unwrap_or(false),
    };
    liteparse::search::search_items(&rust_items, &options)
        .iter()
        .map(JsTextItem::from_rust)
        .collect()
}
