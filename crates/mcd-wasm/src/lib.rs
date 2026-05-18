//! WebAssembly entry points for byte-oriented MCD parsing.
//!
//! The exported ABI is intentionally small and host-neutral: callers copy MCD
//! archive bytes into linear memory, call an operation, then read the last
//! output buffer as UTF-8.

use std::{cell::RefCell, slice};

use mcd_core::{
    Diagnostic, McdError, McdPackage,
    document::McdDocument,
    errors::DiagnosticLevel,
    export::{annotation_export, expanded_markdown_export, original_markdown_export},
    validate::{ValidationResult, validate_package},
};
use serde::Serialize;

thread_local! {
    static LAST_OUTPUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// Allocate `len` bytes in WASM linear memory and return the pointer.
#[unsafe(no_mangle)]
pub extern "C" fn mcd_alloc(len: usize) -> *mut u8 {
    let mut buffer = Vec::<u8>::with_capacity(len);
    let ptr = buffer.as_mut_ptr();
    std::mem::forget(buffer);
    ptr
}

/// Free a pointer previously returned by [`mcd_alloc`].
///
/// `len` must match the length used for allocation.
///
/// # Safety
///
/// `ptr` must be a pointer returned by [`mcd_alloc`] with the same `len`, and it
/// must not have already been freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mcd_free(ptr: *mut u8, len: usize) {
    if ptr.is_null() || len == 0 {
        return;
    }
    unsafe {
        drop(Vec::from_raw_parts(ptr, 0, len));
    }
}

/// Return the pointer to the last operation output buffer.
#[unsafe(no_mangle)]
pub extern "C" fn mcd_output_ptr() -> *const u8 {
    LAST_OUTPUT.with(|output| output.borrow().as_ptr())
}

/// Return the byte length of the last operation output buffer.
#[unsafe(no_mangle)]
pub extern "C" fn mcd_output_len() -> usize {
    LAST_OUTPUT.with(|output| output.borrow().len())
}

/// Validate an MCD package from bytes.
///
/// The output buffer contains a serialized `ValidationResult`. This function
/// always returns `0`; parser errors are represented as invalid validation
/// results with one structured diagnostic.
///
/// # Safety
///
/// `ptr` must point to `len` readable bytes in WASM linear memory for the
/// duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mcd_validate(ptr: *const u8, len: usize) -> i32 {
    let bytes = unsafe { input_bytes(ptr, len) };
    let result = match package_from_bytes(bytes).and_then(|package| validate_package(&package)) {
        Ok(result) => result,
        Err(err) => ValidationResult {
            valid: false,
            diagnostics: vec![diagnostic_from_error(&err)],
        },
    };
    set_json_output(&result)
}

/// Parse the canonical document block stream from MCD package bytes.
///
/// On success, the output buffer contains a JSON array of document blocks and
/// the return value is `0`. On failure, the output contains an error payload and
/// the return value is `1`.
///
/// # Safety
///
/// `ptr` must point to `len` readable bytes in WASM linear memory for the
/// duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mcd_blocks(ptr: *const u8, len: usize) -> i32 {
    let bytes = unsafe { input_bytes(ptr, len) };
    match document_from_bytes(bytes) {
        Ok(document) => set_json_output(&document.blocks),
        Err(err) => set_error_output(&err),
    }
}

/// Export annotation metadata from MCD package bytes.
///
/// On success, the output buffer contains a JSON annotation export object and
/// the return value is `0`. On failure, the output contains an error payload and
/// the return value is `1`.
///
/// # Safety
///
/// `ptr` must point to `len` readable bytes in WASM linear memory for the
/// duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mcd_annotations(ptr: *const u8, len: usize) -> i32 {
    let bytes = unsafe { input_bytes(ptr, len) };
    match package_from_bytes(bytes).and_then(|package| annotation_export(&package)) {
        Ok(annotations) => set_json_output(&annotations),
        Err(err) => set_error_output(&err),
    }
}

/// Export Markdown from MCD package bytes.
///
/// Set `expand_tables` to `1` to render table, chart, and image placements into
/// expanded Markdown. Set it to `0` to return the original entrypoint Markdown.
/// On success, the output buffer contains UTF-8 Markdown and the return value is
/// `0`. On failure, the output contains an error payload and the return value is
/// `1`.
///
/// # Safety
///
/// `ptr` must point to `len` readable bytes in WASM linear memory for the
/// duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mcd_markdown(ptr: *const u8, len: usize, expand_tables: i32) -> i32 {
    let bytes = unsafe { input_bytes(ptr, len) };
    let markdown = package_from_bytes(bytes).and_then(|package| {
        if expand_tables == 0 {
            original_markdown_export(&package)
        } else {
            expanded_markdown_export(&package)
        }
    });
    match markdown {
        Ok(markdown) => set_output(markdown.into_bytes(), 0),
        Err(err) => set_error_output(&err),
    }
}

unsafe fn input_bytes<'a>(ptr: *const u8, len: usize) -> &'a [u8] {
    if len == 0 {
        return &[];
    }
    unsafe { slice::from_raw_parts(ptr, len) }
}

fn package_from_bytes(bytes: &[u8]) -> mcd_core::Result<McdPackage> {
    McdPackage::from_bytes(bytes)
}

fn document_from_bytes(bytes: &[u8]) -> mcd_core::Result<McdDocument> {
    let package = package_from_bytes(bytes)?;
    let manifest = package.manifest()?;
    McdDocument::from_package(&package, &manifest)
}

fn set_json_output(value: &impl Serialize) -> i32 {
    match serde_json::to_vec(value) {
        Ok(bytes) => set_output(bytes, 0),
        Err(err) => set_error_output(&McdError::from(err)),
    }
}

fn set_error_output(err: &McdError) -> i32 {
    let payload = ErrorPayload {
        diagnostic: diagnostic_from_error(err),
    };
    match serde_json::to_vec(&payload) {
        Ok(bytes) => set_output(bytes, 1),
        Err(_) => set_output(
            br#"{"diagnostic":{"level":"error","code":"wasm.output.failed","message":"Failed to serialize WASM error output."}}"#.to_vec(),
            1,
        ),
    }
}

fn set_output(bytes: Vec<u8>, status: i32) -> i32 {
    LAST_OUTPUT.with(|output| {
        *output.borrow_mut() = bytes;
    });
    status
}

fn diagnostic_from_error(err: &McdError) -> Diagnostic {
    err.diagnostic().cloned().unwrap_or_else(|| Diagnostic {
        level: DiagnosticLevel::Error,
        code: "package.parse.failed".to_string(),
        message: err.to_string(),
        source: None,
        related: Vec::new(),
    })
}

#[derive(Serialize)]
struct ErrorPayload {
    diagnostic: Diagnostic,
}
