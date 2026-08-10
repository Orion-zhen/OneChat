mod archive;
mod docx;
mod pdf;
mod pptx;
mod xlsx;

pub(crate) use archive::*;
pub(crate) use docx::*;
pub(crate) use pdf::*;
pub(crate) use pptx::*;
pub(crate) use xlsx::*;

pub(crate) fn png_bytes() -> Vec<u8> {
    b"\x89PNG\r\n\x1a\n".to_vec()
}

pub(crate) fn markdown_cells(line: &str) -> Vec<&str> {
    line.trim_matches('|').split('|').map(str::trim).collect()
}
