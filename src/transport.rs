//! Transport adapters for reading and writing bytes through URI-like targets.
//!
//! The router currently supports:
//! - local filesystem paths and `file://` URIs via [`LocalAdapter`],
//! - `http://` / `https://` endpoints via [`HttpAdapter`], and
//! - `s3://bucket/key` objects via [`S3Adapter`].

use reqwest::blocking::{Client, Response};
use reqwest::header::CONTENT_TYPE;
use std::fmt;
use std::fs::{self, File};
use std::io::{Cursor, Read, Write};
use std::path::Component;
use std::path::PathBuf;
use url::Url;

const S3_ROOT_ENV: &str = "RENDER_SLIDES_S3_ROOT";

#[derive(Debug)]
/// Error type returned by transport adapters and routing helpers.
pub enum TransportError {
    /// URI parsing or path conversion failed.
    InvalidUri(String),
    /// URI scheme is not supported by the current router configuration.
    UnsupportedScheme(String),
    /// Local filesystem I/O failed.
    Io(std::io::Error),
    /// HTTP client request failed before receiving a successful response.
    Http(reqwest::Error),
    /// HTTP request returned a non-success status code.
    HttpStatus {
        /// HTTP method (or method strategy) attempted.
        method: &'static str,
        /// Destination URI.
        uri: String,
        /// Returned status code.
        status: u16,
    },
}

/// Converts transport errors into user-facing error messages.
impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidUri(uri) => write!(f, "Invalid URI or path: {uri}"),
            Self::UnsupportedScheme(scheme) => write!(f, "Unsupported URI scheme: {scheme}"),
            Self::Io(err) => write!(f, "I/O error: {err}"),
            Self::Http(err) => write!(f, "HTTP transport error: {err}"),
            Self::HttpStatus {
                method,
                uri,
                status,
            } => {
                write!(f, "HTTP {method} {uri} failed with status {status}")
            }
        }
    }
}

impl std::error::Error for TransportError {}

impl From<std::io::Error> for TransportError {
    /// Wraps a standard I/O error in the transport error type.
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<reqwest::Error> for TransportError {
    /// Wraps an HTTP client error in the transport error type.
    fn from(value: reqwest::Error) -> Self {
        Self::Http(value)
    }
}

/// Opens byte streams for reading from a URI-like source.
pub trait Source {
    /// Opens a readable stream for the given URI.
    fn open_read(&self, uri: &str) -> Result<Box<dyn Read + Send>, TransportError>;
}

/// Opens byte streams for writing to a URI-like sink.
pub trait Sink {
    /// Opens a writable stream for the given URI.
    fn open_write(&self, uri: &str) -> Result<Box<dyn Write + Send>, TransportError>;
}

#[derive(Clone, Default)]
/// Local filesystem transport adapter.
pub struct LocalAdapter;

impl Source for LocalAdapter {
    /// Opens a local file for reading from either a raw path or file:// URI.
    fn open_read(&self, uri: &str) -> Result<Box<dyn Read + Send>, TransportError> {
        let path = resolve_local_path(uri)?;
        Ok(Box::new(File::open(path)?))
    }
}

impl Sink for LocalAdapter {
    /// Creates or truncates a local file for writing at the resolved path.
    fn open_write(&self, uri: &str) -> Result<Box<dyn Write + Send>, TransportError> {
        let path = resolve_local_path(uri)?;
        Ok(Box::new(File::create(path)?))
    }
}

#[derive(Clone)]
/// Blocking HTTP transport adapter used for GET/PUT/POST operations.
pub struct HttpAdapter {
    client: Client,
}

impl HttpAdapter {
    /// Creates an HTTP adapter with an explicit no-proxy client configuration.
    pub fn new() -> Self {
        Self {
            client: Client::builder().no_proxy().build().expect("http client"),
        }
    }
}

impl Default for HttpAdapter {
    /// Builds the default HTTP adapter.
    fn default() -> Self {
        Self::new()
    }
}

impl Source for HttpAdapter {
    /// Performs an HTTP GET and returns the response body as a readable stream.
    fn open_read(&self, uri: &str) -> Result<Box<dyn Read + Send>, TransportError> {
        let response = self.client.get(uri).send()?;
        if !response.status().is_success() {
            return Err(TransportError::HttpStatus {
                method: "GET",
                uri: uri.to_string(),
                status: response.status().as_u16(),
            });
        }

        Ok(Box::new(response))
    }
}

impl Sink for HttpAdapter {
    /// Returns a buffered writer that uploads data to the target URI on flush.
    fn open_write(&self, uri: &str) -> Result<Box<dyn Write + Send>, TransportError> {
        Ok(Box::new(HttpWriteBuffer::new(
            self.client.clone(),
            uri.to_string(),
        )))
    }
}

struct HttpWriteBuffer {
    client: Client,
    uri: String,
    buffer: Cursor<Vec<u8>>,
    uploaded: bool,
}

impl HttpWriteBuffer {
    /// Creates a buffered HTTP sink that uploads on first flush.
    fn new(client: Client, uri: String) -> Self {
        Self {
            client,
            uri,
            buffer: Cursor::new(Vec::new()),
            uploaded: false,
        }
    }

    /// Uploads buffered bytes via PUT first, then falls back to POST.
    fn flush_http(&self) -> Result<Response, TransportError> {
        let body = self.buffer.get_ref().clone();

        let put_response = self
            .client
            .put(&self.uri)
            .header(CONTENT_TYPE, "application/octet-stream")
            .body(body.clone())
            .send()?;

        if put_response.status().is_success() {
            return Ok(put_response);
        }

        let post_response = self
            .client
            .post(&self.uri)
            .header(CONTENT_TYPE, "application/octet-stream")
            .body(body)
            .send()?;

        if post_response.status().is_success() {
            return Ok(post_response);
        }

        Err(TransportError::HttpStatus {
            method: "PUT/POST",
            uri: self.uri.clone(),
            status: post_response.status().as_u16(),
        })
    }
}

impl Write for HttpWriteBuffer {
    /// Appends bytes to the in-memory upload buffer before finalization.
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.uploaded {
            return Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "HTTP sink is already finalized; cannot write after flush",
            ));
        }

        self.buffer.write(buf)
    }

    /// Finalizes the sink by uploading buffered content exactly once.
    fn flush(&mut self) -> std::io::Result<()> {
        if self.uploaded {
            return Ok(());
        }

        self.flush_http()
            .map_err(|err| std::io::Error::other(err.to_string()))?;
        self.uploaded = true;

        Ok(())
    }
}

impl Drop for HttpWriteBuffer {
    /// Attempts a best-effort upload if the buffer is dropped before flushing.
    fn drop(&mut self) {
        if !self.uploaded {
            let _ = self.flush();
        }
    }
}

#[derive(Clone)]
/// `s3://bucket/key` adapter backed by a configurable local object-store root.
///
/// This adapter is intentionally local-file-backed scaffolding for now. It
/// resolves S3 URIs to `{root}/{bucket}/{key}` and performs filesystem I/O.
pub struct S3Adapter {
    root: PathBuf,
}

impl S3Adapter {
    /// Creates an S3 adapter using `RENDER_SLIDES_S3_ROOT` when set.
    ///
    /// Falls back to `.render_slides_s3` under the current working directory.
    pub fn new() -> Self {
        let root = std::env::var_os(S3_ROOT_ENV)
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(".render_slides_s3"));
        Self { root }
    }

    /// Resolves an S3 URI into its local backing path.
    fn resolve_path(&self, uri: &str) -> Result<PathBuf, TransportError> {
        let (bucket, key) = parse_s3_uri(uri)?;
        Ok(self.root.join(bucket).join(key))
    }
}

impl Default for S3Adapter {
    /// Builds the default S3 adapter.
    fn default() -> Self {
        Self::new()
    }
}

impl Source for S3Adapter {
    /// Opens a local backing file for reading from an S3 URI.
    fn open_read(&self, uri: &str) -> Result<Box<dyn Read + Send>, TransportError> {
        let path = self.resolve_path(uri)?;
        Ok(Box::new(File::open(path)?))
    }
}

impl Sink for S3Adapter {
    /// Opens a local backing file for writing to an S3 URI, creating parents.
    fn open_write(&self, uri: &str) -> Result<Box<dyn Write + Send>, TransportError> {
        let path = self.resolve_path(uri)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        Ok(Box::new(File::create(path)?))
    }
}

#[derive(Clone, Default)]
/// Routes transport operations to local, HTTP, or S3 adapters by URI scheme.
pub struct TransportRouter {
    local: LocalAdapter,
    http: HttpAdapter,
    s3: S3Adapter,
}

impl TransportRouter {
    /// Builds a router with local filesystem, HTTP(S), and S3 adapters.
    pub fn new() -> Self {
        Self {
            local: LocalAdapter,
            http: HttpAdapter::new(),
            s3: S3Adapter::new(),
        }
    }

    /// Opens a readable stream based on URI scheme dispatch.
    pub fn open_read(&self, uri: &str) -> Result<Box<dyn Read + Send>, TransportError> {
        let scheme = scheme(uri)?;

        match scheme.as_str() {
            "file" | "" => self.local.open_read(uri),
            "http" | "https" => self.http.open_read(uri),
            "s3" => self.s3.open_read(uri),
            _ => Err(TransportError::UnsupportedScheme(scheme)),
        }
    }

    /// Opens a writable stream based on URI scheme dispatch.
    pub fn open_write(&self, uri: &str) -> Result<Box<dyn Write + Send>, TransportError> {
        let scheme = scheme(uri)?;

        match scheme.as_str() {
            "file" | "" => self.local.open_write(uri),
            "http" | "https" => self.http.open_write(uri),
            "s3" => self.s3.open_write(uri),
            _ => Err(TransportError::UnsupportedScheme(scheme)),
        }
    }
}

/// Determines the scheme for a URI or returns empty for local paths.
fn scheme(uri: &str) -> Result<String, TransportError> {
    if uri.contains("://") {
        let parsed = Url::parse(uri).map_err(|_| TransportError::InvalidUri(uri.to_string()))?;
        return Ok(parsed.scheme().to_string());
    }

    Ok("".to_string())
}

/// Resolves a local filesystem path from a raw path or file:// URI.
fn resolve_local_path(uri_or_path: &str) -> Result<PathBuf, TransportError> {
    if uri_or_path.starts_with("file://") {
        let parsed = Url::parse(uri_or_path)
            .map_err(|_| TransportError::InvalidUri(uri_or_path.to_string()))?;
        let path = parsed
            .to_file_path()
            .map_err(|_| TransportError::InvalidUri(uri_or_path.to_string()))?;
        return Ok(path);
    }

    Ok(PathBuf::from(uri_or_path))
}

/// Parses `s3://bucket/key` URIs into bucket and key components.
fn parse_s3_uri(uri: &str) -> Result<(String, String), TransportError> {
    let parsed = Url::parse(uri).map_err(|_| TransportError::InvalidUri(uri.to_string()))?;
    if parsed.scheme() != "s3" {
        return Err(TransportError::UnsupportedScheme(
            parsed.scheme().to_string(),
        ));
    }

    let bucket = parsed
        .host_str()
        .ok_or_else(|| TransportError::InvalidUri(uri.to_string()))?;

    let uri_without_scheme = uri
        .strip_prefix("s3://")
        .ok_or_else(|| TransportError::InvalidUri(uri.to_string()))?;
    let (_, remainder) = uri_without_scheme
        .split_once('/')
        .ok_or_else(|| TransportError::InvalidUri(uri.to_string()))?;
    let key = remainder
        .split(['?', '#'])
        .next()
        .unwrap_or_default()
        .trim_start_matches('/');

    if key.is_empty() {
        return Err(TransportError::InvalidUri(uri.to_string()));
    }

    let mut normalized_segments = Vec::new();
    if key.contains('\\') {
        return Err(TransportError::InvalidUri(uri.to_string()));
    }

    for component in PathBuf::from(key).components() {
        match component {
            Component::Normal(segment) => {
                let as_str = segment
                    .to_str()
                    .ok_or_else(|| TransportError::InvalidUri(uri.to_string()))?;
                normalized_segments.push(as_str.to_string());
            }
            Component::CurDir
            | Component::ParentDir
            | Component::RootDir
            | Component::Prefix(_) => {
                return Err(TransportError::InvalidUri(uri.to_string()));
            }
        }
    }

    if normalized_segments.is_empty() {
        return Err(TransportError::InvalidUri(uri.to_string()));
    }

    Ok((bucket.to_string(), normalized_segments.join("/")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn local_roundtrip_supports_plain_paths() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let output_path = temp_dir.path().join("data.bin");

        let router = TransportRouter::new();
        {
            let mut writer = router
                .open_write(output_path.to_str().expect("path str"))
                .expect("open write");
            writer.write_all(b"hello").expect("write");
        }

        let mut read_back = String::new();
        router
            .open_read(output_path.to_str().expect("path str"))
            .expect("open read")
            .read_to_string(&mut read_back)
            .expect("read");

        assert_eq!(read_back, "hello");
    }

    #[test]
    fn local_roundtrip_supports_file_uri() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let output_path = temp_dir.path().join("uri-data.bin");
        let uri = format!("file://{}", output_path.display());

        let router = TransportRouter::new();
        {
            let mut writer = router.open_write(&uri).expect("open write");
            writer.write_all(b"world").expect("write");
        }

        let mut read_back = String::new();
        router
            .open_read(&uri)
            .expect("open read")
            .read_to_string(&mut read_back)
            .expect("read");

        assert_eq!(read_back, "world");
    }

    #[test]
    fn s3_uri_parsing_extracts_bucket_and_key() {
        let (bucket, key) = parse_s3_uri("s3://slides-bucket/path/to/deck.json").expect("s3 uri");
        assert_eq!(bucket, "slides-bucket");
        assert_eq!(key, "path/to/deck.json");
    }

    #[test]
    fn s3_roundtrip_uses_configured_local_root() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        std::env::set_var(S3_ROOT_ENV, temp_dir.path());

        let router = TransportRouter::new();
        {
            let mut writer = router
                .open_write("s3://slides-bucket/path/data.bin")
                .expect("s3 write");
            writer.write_all(b"payload").expect("write");
        }

        let mut read_back = String::new();
        router
            .open_read("s3://slides-bucket/path/data.bin")
            .expect("s3 read")
            .read_to_string(&mut read_back)
            .expect("read");

        std::env::remove_var(S3_ROOT_ENV);
        assert_eq!(read_back, "payload");
    }

    #[test]
    fn s3_uri_rejects_parent_dir_traversal() {
        let err = parse_s3_uri("s3://slides-bucket/../../outside.txt").expect_err("invalid uri");
        assert!(matches!(err, TransportError::InvalidUri(_)));
    }

    #[test]
    fn http_write_surfaces_non_success_status() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");

        thread::spawn(move || {
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().expect("accept");
                let mut req_buf = [0_u8; 1024];
                let _ = stream.read(&mut req_buf);

                let response = "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
                stream
                    .write_all(response.as_bytes())
                    .expect("write response");
            }
        });

        let router = TransportRouter::new();
        let mut writer = router
            .open_write(&format!("http://{addr}"))
            .expect("open http write");

        writer.write_all(b"payload").expect("buffer write");
        let err = writer.flush().expect_err("expected flush failure");
        assert!(err.to_string().contains("HTTP PUT/POST"));
    }

    #[test]
    fn http_write_rejects_writes_after_flush() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");

        thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut req_buf = [0_u8; 1024];
            let _ = stream.read(&mut req_buf);

            let response = "HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
            stream
                .write_all(response.as_bytes())
                .expect("write response");
        });

        let router = TransportRouter::new();
        let mut writer = router
            .open_write(&format!("http://{addr}"))
            .expect("open http write");

        writer.write_all(b"payload").expect("buffer write");
        writer.flush().expect("flush");

        let err = writer
            .write_all(b"extra")
            .expect_err("writes after flush should fail");
        assert_eq!(err.kind(), std::io::ErrorKind::BrokenPipe);
    }

    #[test]
    fn http_get_reads_payload() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");

        thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut req_buf = [0_u8; 1024];
            let _ = stream.read(&mut req_buf);

            let body = b"payload";
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            stream
                .write_all(response.as_bytes())
                .expect("write headers");
            stream.write_all(body).expect("write body");
        });

        let router = TransportRouter::new();
        let mut response = String::new();
        router
            .open_read(&format!("http://{addr}"))
            .expect("open http read")
            .read_to_string(&mut response)
            .expect("read");

        assert_eq!(response, "payload");
    }
}
