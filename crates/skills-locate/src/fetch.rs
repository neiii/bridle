use std::io::{Cursor, Read};
use std::thread;
use std::time::Duration;

use serde::de::DeserializeOwned;
use zip::ZipArchive;

use crate::{Error, Result};

const MAX_RETRIES: u32 = 3;
const RETRY_DELAYS_MS: [u64; 3] = [100, 500, 2000];
const SIZE_LIMIT: u64 = 200 * 1024 * 1024;

pub fn fetch_bytes(url: &str) -> Result<Vec<u8>> {
    let mut last_error = None;

    for attempt in 0..MAX_RETRIES {
        match try_fetch(url) {
            Ok(bytes) => return Ok(bytes),
            Err(e) => {
                if is_retryable(&e) && attempt < MAX_RETRIES - 1 {
                    thread::sleep(Duration::from_millis(RETRY_DELAYS_MS[attempt as usize]));
                    last_error = Some(e);
                } else {
                    return Err(e);
                }
            }
        }
    }

    Err(last_error.unwrap_or_else(|| Error::Http("max retries exceeded".into())))
}

fn try_fetch(url: &str) -> Result<Vec<u8>> {
    let mut response = ureq::get(url).call().map_err(|e| match e {
        ureq::Error::StatusCode(code) => Error::Http(format!("HTTP {code} for {url}")),
        ureq::Error::Io(io_err) => Error::Http(format!("transport error: {io_err}")),
        _ => Error::Http(format!("request failed: {e}")),
    })?;

    // Check content-length header before reading body
    if let Some(len) = response.headers().get("content-length")
        && let Ok(size) = len.to_str().unwrap_or("").parse::<u64>()
        && size > SIZE_LIMIT
    {
        return Err(Error::SizeLimit {
            size,
            limit: SIZE_LIMIT,
        });
    }

    // ureq 3.x: must use body_mut().with_config().limit() to override 10MB default
    let bytes = response
        .body_mut()
        .with_config()
        .limit(SIZE_LIMIT)
        .read_to_vec()
        .map_err(|e| Error::Http(format!("read error: {e}")))?;

    if bytes.len() as u64 > SIZE_LIMIT {
        return Err(Error::SizeLimit {
            size: bytes.len() as u64,
            limit: SIZE_LIMIT,
        });
    }

    Ok(bytes)
}

fn is_retryable(e: &Error) -> bool {
    match e {
        Error::Http(msg) => {
            msg.contains("transport")
                || msg.contains("HTTP 5")
                || msg.contains("timeout")
                || msg.contains("connection")
        }
        _ => false,
    }
}

pub fn fetch_json<T: DeserializeOwned>(url: &str) -> Result<T> {
    let bytes = fetch_bytes(url)?;
    let text = String::from_utf8(bytes).map_err(|e| Error::Http(format!("invalid UTF-8: {e}")))?;
    serde_json::from_str(&text).map_err(Error::from)
}

pub fn extract_file(zip_bytes: &[u8], path: &str) -> Result<String> {
    let cursor = Cursor::new(zip_bytes);
    let mut archive =
        ZipArchive::new(cursor).map_err(|e| Error::ZipExtract(format!("invalid ZIP: {e}")))?;

    let normalized_path = path.trim_start_matches('/');

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| Error::ZipExtract(format!("read entry: {e}")))?;

        let name = file.name().to_string();
        if name.ends_with(normalized_path) || name == normalized_path {
            let mut content = String::new();
            file.read_to_string(&mut content)
                .map_err(|e| Error::ZipExtract(format!("read file: {e}")))?;
            return Ok(content);
        }
    }

    Err(Error::NotFound(path.to_string()))
}

pub fn list_files(zip_bytes: &[u8], suffix: &str) -> Result<Vec<String>> {
    let cursor = Cursor::new(zip_bytes);
    let archive =
        ZipArchive::new(cursor).map_err(|e| Error::ZipExtract(format!("invalid ZIP: {e}")))?;

    let matches: Vec<String> = archive
        .file_names()
        .filter(|name| name.ends_with(suffix))
        .map(String::from)
        .collect();

    Ok(matches)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn create_test_zip(files: &[(&str, &str)]) -> Vec<u8> {
        let mut buf = Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut buf);
            let options = zip::write::SimpleFileOptions::default();
            for (name, content) in files {
                zip.start_file(*name, options).unwrap();
                zip.write_all(content.as_bytes()).unwrap();
            }
            zip.finish().unwrap();
        }
        buf.into_inner()
    }

    #[test]
    fn extract_file_found() {
        let zip = create_test_zip(&[("test.txt", "hello world")]);
        let content = extract_file(&zip, "test.txt").unwrap();
        assert_eq!(content, "hello world");
    }

    #[test]
    fn extract_file_nested() {
        let zip = create_test_zip(&[("repo-main/path/to/file.md", "content here")]);
        let content = extract_file(&zip, "path/to/file.md").unwrap();
        assert_eq!(content, "content here");
    }

    #[test]
    fn extract_file_not_found() {
        let zip = create_test_zip(&[("other.txt", "data")]);
        let result = extract_file(&zip, "missing.txt");
        assert!(matches!(result, Err(Error::NotFound(_))));
    }

    #[test]
    fn list_files_by_suffix() {
        let zip = create_test_zip(&[("a/SKILL.md", ""), ("b/SKILL.md", ""), ("c/README.md", "")]);
        let matches = list_files(&zip, "SKILL.md").unwrap();
        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn list_files_empty() {
        let zip = create_test_zip(&[("file.txt", "")]);
        let matches = list_files(&zip, ".json").unwrap();
        assert!(matches.is_empty());
    }
}
