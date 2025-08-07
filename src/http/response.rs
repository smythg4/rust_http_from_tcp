use crate::{http::headers::Headers};
use tokio::net::TcpStream;
use tokio::io::AsyncWriteExt;

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum StatusCode {
    StatusOk,
    StatusBadRequest,
    StatusInternalServerError,
}

impl std::fmt::Display for StatusCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StatusCode::StatusOk => write!(f, "HTTP/1.1 200 OK"),
            StatusCode::StatusBadRequest  => write!(f, "HTTP/1.1 400 Bad Request"),
            StatusCode::StatusInternalServerError => write!(f, "HTTP/1.1 500 Internal Server Error"),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum WriterState {
    New,
    StatusWritten,
    HeadersWritten,
    BodyWritten,
}

pub struct Writer {
    stream: TcpStream,
    state: WriterState,
}

impl Writer {
    pub fn new(stream: TcpStream) -> Self {
        Writer {
            stream,
            state: WriterState::New,
        }
    }

    pub async fn write_status_line(&mut self, status_code: StatusCode) -> Result<(), std::io::Error> {
        if self.state != WriterState::New {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Status line must be written first"
            ));
        }

        self.stream.write_all(&format!("{}\r\n", status_code).as_bytes()).await?;
        self.state = WriterState::StatusWritten;
        Ok(())
    }

    pub async fn write_headers(&mut self, headers: &Headers) -> Result<(), std::io::Error> {
        if self.state != WriterState::StatusWritten {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Headers must be written after status line"
            ));
        }

        self.stream.write_all(&format!("{}\r\n\r\n", headers).as_bytes()).await?;
        self.state = WriterState::HeadersWritten;
        Ok(())
    }

    pub async fn write_body(&mut self, body: &[u8]) -> Result<usize, std::io::Error> {
        if self.state != WriterState::HeadersWritten {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Body must be written after headers"
            ));
        }

        self.stream.write_all(body).await?;
        self.stream.flush().await?;
        self.state = WriterState::BodyWritten;
        Ok(body.len())
    }
}

pub struct Response {
    status_line: StatusCode,
    pub headers: Headers,
    pub body: Vec<u8>,
}

impl Default for Response {
    fn default() -> Self {
        Response {
            status_line: StatusCode::StatusOk,
            headers: Self::get_default_headers(0),
            body: Vec::new(),
        }
    }
}

impl std::fmt::Display for Response {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}\r\n{}\r\n\r\n{}\r\n",
            self.status_line,
            self.headers,
            String::from_utf8_lossy(&self.body)
        )
    }
}

impl Response {

    pub fn new(status_code: StatusCode, body: Vec<u8>) -> Self {
        Response {
            status_line: status_code,
            headers: Self::get_default_headers(body.len()),
            body,
        }
    }

    pub fn get_default_headers(content_len: usize) -> Headers {
        let mut result = Headers::new();
        result.insert("Content-Length".to_string(), content_len.to_string());
        result.insert("Connection".to_string(), "close".to_string());
        result.insert("Content-Type".to_string(), "text/plain".to_string());

        result
    }

    pub fn set_body(&mut self, body: Vec<u8>) {
        let content_length = body.len();
        self.body = body;
        self.headers.insert("content-length".to_string(), content_length.to_string());
    }
}