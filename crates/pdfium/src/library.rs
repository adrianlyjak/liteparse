use std::ffi::CString;
use std::sync::Once;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::{Mutex, MutexGuard, OnceLock};

use crate::document::{Document, RetainedDocument};
use crate::error::PdfiumError;
use crate::ffi;

static INIT: Once = Once::new();

/// Process-global PDFium serialization lock.
///
/// PDFium's FFI is **not thread-safe**: concurrent calls (even across distinct
/// documents) corrupt internal state and cause heap UB (double-free / heap
/// corruption). Every [`Library`] handle holds this mutex for its entire
/// lifetime, and the owning PDFium resources ([`Document`], `Page`,
/// `TextPage`, `Bitmap`) borrow from a [`Library`] via their `'lib` lifetime,
/// so the borrow checker statically prevents PDFium work outside the lock.
/// (`Font` is a borrowed, non-owning handle constructed through an `unsafe`
/// fn; its lock discipline is the caller's responsibility, not statically
/// enforced.)
#[cfg(not(target_arch = "wasm32"))]
fn pdfium_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// A live, locked PDFium session.
///
/// Holding a `Library` proves the current thread has exclusive,
/// process-wide access to PDFium. All PDFium resources ([`Document`] etc.)
/// borrow from this handle, which makes it impossible to call into PDFium
/// without first acquiring the lock.
///
/// `Library` is intentionally **not `Clone`**. To use PDFium from a
/// different scope, call [`Library::init`] again — this will block until
/// any other in-flight PDFium work has finished.
///
/// On `wasm32` there is no threading, so the lock is elided.
///
/// The snippet below must fail to compile — a `Document` cannot outlive
/// the `Library` that opened it:
///
/// ```compile_fail
/// use liteparse_pdfium::{Library, Document};
/// let doc: Document<'static> = {
///     let lib = Library::init();
///     lib.load_document("x.pdf", None).unwrap()
/// };
/// // `lib` was dropped above — using `doc` here is a use-after-unlock.
/// let _ = doc.page_count();
/// ```
pub struct Library {
    #[cfg(not(target_arch = "wasm32"))]
    _guard: MutexGuard<'static, ()>,
    #[cfg(target_arch = "wasm32")]
    _private: (),
}

impl Library {
    /// Acquire the process-wide PDFium lock, blocking the current thread
    /// until any other in-flight PDFium work has finished. Initializes the
    /// library on first call.
    ///
    /// Multiple concurrent callers are serialized; only one `Library`
    /// instance exists at a time.
    pub fn init() -> Library {
        #[cfg(not(target_arch = "wasm32"))]
        {
            pdfium_sys::dynamic::load_default().expect("failed to load pdfium shared library");
            // Recover from poisoning: a panic mid-FFI may leave PDFium in
            // an odd state, but subsequent calls should still be allowed
            // (the worst case is that the next parse also fails cleanly).
            let guard = pdfium_lock()
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            INIT.call_once(|| unsafe { ffi!(FPDF_InitLibrary()) });
            Library { _guard: guard }
        }
        #[cfg(target_arch = "wasm32")]
        {
            INIT.call_once(|| unsafe { ffi!(FPDF_InitLibrary()) });
            Library { _private: () }
        }
    }

    pub fn load_document(
        &self,
        path: &str,
        password: Option<&str>,
    ) -> Result<Document<'_>, PdfiumError> {
        let c_path = CString::new(path).map_err(|_| PdfiumError::FileNotFound)?;
        let c_password = password
            .map(|p| CString::new(p).map_err(|_| PdfiumError::OperationFailed))
            .transpose()?;

        let handle = unsafe {
            ffi!(FPDF_LoadDocument(
                c_path.as_ptr(),
                c_password.as_ref().map_or(std::ptr::null(), |p| p.as_ptr()),
            ))
        };

        if handle.is_null() {
            return Err(PdfiumError::from_last_error());
        }

        Ok(Document {
            handle,
            owns_handle: true,
            _lib: std::marker::PhantomData,
        })
    }

    /// Load a PDF from a borrowed byte buffer.
    ///
    /// PDFium reads from the buffer lazily, so the returned document borrows
    /// both this locked library and `data` for the same lifetime:
    ///
    /// ```compile_fail
    /// use liteparse_pdfium::Library;
    /// let library = Library::init();
    /// let document = {
    ///     let data = vec![0_u8; 8];
    ///     library.load_document_from_bytes(&data, None).unwrap()
    /// };
    /// let _ = document.page_count();
    /// ```
    pub fn load_document_from_bytes<'document>(
        &'document self,
        data: &'document [u8],
        password: Option<&str>,
    ) -> Result<Document<'document>, PdfiumError> {
        let c_password = password
            .map(|p| CString::new(p).map_err(|_| PdfiumError::OperationFailed))
            .transpose()?;

        let handle = unsafe {
            ffi!(FPDF_LoadMemDocument(
                data.as_ptr() as *const std::ffi::c_void,
                data.len() as i32,
                c_password.as_ref().map_or(std::ptr::null(), |p| p.as_ptr()),
            ))
        };

        if handle.is_null() {
            return Err(PdfiumError::from_last_error());
        }

        Ok(Document {
            handle,
            owns_handle: true,
            _lib: std::marker::PhantomData,
        })
    }

    /// Borrow a detached document for this locked PDFium transaction.
    pub fn reborrow_document<'lib>(&'lib self, retained: &'lib RetainedDocument) -> Document<'lib> {
        Document {
            handle: retained.handle,
            owns_handle: false,
            _lib: std::marker::PhantomData,
        }
    }

    /// Close a detached document while holding the process-global PDFium lock.
    pub fn close_retained_document(&self, retained: RetainedDocument) {
        unsafe { ffi!(FPDF_CloseDocument(retained.handle)) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn retained_document_moves_between_serialized_transactions() {
        fn require_send<T: Send>() {}
        require_send::<RetainedDocument>();

        let bytes = include_bytes!("../../../integration_tests_data/sample.pdf");
        let retained = {
            let library = Library::init();
            let document = library.load_document_from_bytes(bytes, None).unwrap();
            assert!(document.page_count() > 0);
            // SAFETY: `bytes` has static storage and the spawned thread closes
            // the handle through a newly locked `Library`.
            unsafe { document.detach().unwrap() }
        };

        std::thread::spawn(move || {
            let library = Library::init();
            assert!(library.reborrow_document(&retained).page_count() > 0);
            library.close_retained_document(retained);
        })
        .join()
        .unwrap();
    }

    #[test]
    fn retained_document_cannot_be_detached_twice() {
        let bytes = include_bytes!("../../../integration_tests_data/sample.pdf");
        let library = Library::init();
        let document = library.load_document_from_bytes(bytes, None).unwrap();
        // SAFETY: `bytes` has static storage and this test closes the handle
        // through the same locked `Library`.
        let retained = unsafe { document.detach().unwrap() };
        let reborrowed = library.reborrow_document(&retained);

        // SAFETY: This deliberately exercises the runtime ownership check.
        assert!(matches!(
            unsafe { reborrowed.detach() },
            Err(PdfiumError::OperationFailed)
        ));
        library.close_retained_document(retained);
    }

    #[test]
    fn retained_document_reborrow_rejects_form_mutation() {
        let bytes = include_bytes!("../../../integration_tests_data/filled_acroform.pdf");
        let library = Library::init();
        let document = library.load_document_from_bytes(bytes, None).unwrap();
        assert!(document.form_environment().is_some());
        // SAFETY: `bytes` has static storage and the retained handle is closed
        // through the same locked `Library` before the test returns.
        let retained = unsafe { document.detach().unwrap() };
        let borrowed = library.reborrow_document(&retained);

        assert!(borrowed.form_environment().is_none());
        assert!(matches!(
            borrowed.flatten_form_widgets(0),
            Err(PdfiumError::OperationFailed)
        ));
        assert!(!borrowed.page(0).unwrap().flatten_form_widgets_for_display());

        drop(borrowed);
        library.close_retained_document(retained);
    }
}
