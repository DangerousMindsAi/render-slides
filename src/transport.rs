use reqwest::blocking::{Client, Response};
use reqwest::header::CONTENT_TYPE;
use std::fmt;
use std::fs::File;
use std::io::{Cursor, Read, Write};
use std::path::PathBuf;
use url::Url;

#[derive(Debug)]
pub enum TransportError {
    InvalidUri(String),
    UnsupportedScheme(String),
    Io(std::io::Error),
    Http(reqwest::Error),
    HttpStatus {
        method: &'static str,
        uri: String,
        status: u16,
    },
}

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
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<reqwest::Error> for TransportError {
    fn from(value: reqwest::Error) -> Self {
        Self::Http(value)
    }
}

pub trait Source {
    fn open_read(&self, uri: &str) -> Result<Box<dyn Read + Send>, TransportError>;
}

pub trait Sink {
    fn open_write(&self, uri: &str) -> Result<Box<dyn Write + Send>, TransportError>;
}

#[derive(Clone, Default)]
pub struct LocalAdapter;

impl Source for LocalAdapter {
    fn open_read(&self, uri: &str) -> Result<Box<dyn Read + Send>, TransportError> {
        let path = resolve_local_path(uri)?;
        Ok(Box::new(File::open(path)?))
    }
}

impl Sink for LocalAdapter {
    fn open_write(&self, uri: &str) -> Result<Box<dyn Write + Send>, TransportError> {
        let path = resolve_local_path(uri)?;
        Ok(Box::new(File::create(path)?))
    }
}

#[derive(Clone)]
pub struct HttpAdapter {
    client: Client,
}

impl HttpAdapter {
    pub fn new() -> Self {
        Self {
            client: Client::builder().no_proxy().build().expect("http client"),
        }
    }
}

impl Default for HttpAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl Source for HttpAdapter {
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
    fn new(client: Client, uri: String) -> Self {
        Self {
            client,
            uri,
            buffer: Cursor::new(Vec::new()),
            uploaded: false,
        }
    }

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
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.uploaded {
            return Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "HTTP sink is already finalized; cannot write after flush",
            ));
        }

        self.buffer.write(buf)
    }

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
    fn drop(&mut self) {
        if !self.uploaded {
            let _ = self.flush();
        }
    }
}

#[derive(Default, Clone)]
pub struct TransportRouter {
    local: LocalAdapter,
    http: HttpAdapter,
}

impl TransportRouter {
    pub fn new() -> Self {
        Self {
            local: LocalAdapter,
            http: HttpAdapter::new(),
        }
    }

    pub fn open_read(&self, uri: &str) -> Result<Box<dyn Read + Send>, TransportError> {
        let scheme = scheme(uri)?;

        match scheme.as_str() {
            "file" | "" => self.local.open_read(uri),
            "http" | "https" => self.http.open_read(uri),
            _ => Err(TransportError::UnsupportedScheme(scheme)),
        }
    }

    pub fn open_write(&self, uri: &str) -> Result<Box<dyn Write + Send>, TransportError> {
        let scheme = scheme(uri)?;

        match scheme.as_str() {
            "file" | "" => self.local.open_write(uri),
            "http" | "https" => self.http.open_write(uri),
            _ => Err(TransportError::UnsupportedScheme(scheme)),
        }
    }
}

fn scheme(uri: &str) -> Result<String, TransportError> {
    if uri.contains("://") {
        let parsed = Url::parse(uri).map_err(|_| TransportError::InvalidUri(uri.to_string()))?;
        return Ok(parsed.scheme().to_string());
    }

    Ok("".to_string())
}

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
    fn rejects_unknown_scheme() {
        let router = TransportRouter::new();
        let result = router.open_read("s3://bucket/key");

        let error = match result {
            Err(error) => error,
            Ok(_) => panic!("expected unsupported scheme error"),
        };

        if let TransportError::UnsupportedScheme(scheme) = error {
            assert_eq!(scheme, "s3");
        } else {
            panic!("expected unsupported scheme error");
        }
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
