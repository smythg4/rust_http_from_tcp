use tokio::io::{AsyncReadExt, AsyncRead};

use crate::http::headers::Headers;

#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    InvalidFormat(String),
    IOError,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::InvalidFormat(s) => write!(f, "Invalid request line format: {}",s),
            ParseError::IOError => write!(f, "Read/write error on the io end"),
        }
    }
}

impl std::error::Error for ParseError {}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
enum ParserState {
    Initialized,
    ParsingHeaders,
    ParsingBody,
    Done,
}

pub struct Request {
    request_line: RequestLine,
    headers: Headers,
    body: Vec<u8>,
    parser_state: ParserState,
}

impl std::fmt::Display for Request {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}\n{}\nBody:\n{}", self.request_line, self.headers, String::from_utf8_lossy(&self.body))
    }
}

impl Request {
    pub fn new() -> Self {
        Request {
            request_line: RequestLine::default(),
            headers: Headers::new(),
            body: Vec::new(),
            parser_state: ParserState::Initialized,
        }
    }

    pub fn get_target(&self) -> &str {
        &self.request_line.request_target
    }

    fn parse_single(&mut self, data: &[u8]) -> Result<usize, ParseError> {

        match self.parser_state {
            ParserState::Initialized => {
                match RequestLine::parse(data) {
                    Ok((Some(request_line), bytes_read)) => {
                        self.request_line = request_line;
                        self.parser_state = ParserState::ParsingHeaders;
                        Ok(bytes_read)
                    },
                    Ok((None, bytes_read)) => {
                        Ok(bytes_read)
                    },
                    Err(e) => Err(e),
                }
            },
            ParserState::ParsingHeaders => {
                match self.headers.parse(data) {
                    Ok((bytes_read, done)) => {
                        if done {
                            match self.headers.get("content-length") {
                                Some(str) => {
                                    match str.parse::<usize>() {
                                        Ok(0) => { self.parser_state = ParserState::Done },
                                        Ok(_) => { self.parser_state = ParserState::ParsingBody },
                                        Err(_) => { return Err(ParseError::InvalidFormat("invalid content-length".to_string())); },
                                    }
                                },
                                None => {
                                    // no content-length, assume there's no body to parse
                                    self.parser_state = ParserState::Done;
                                }
                            }
                        }
                        Ok(bytes_read)
                    },
                    Err(e) => Err(e)
                }
            },
            ParserState::ParsingBody => {
                let content_length = self.headers.get("content-length")
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(0);

                let bytes_needed = content_length - self.body.len();
                let bytes_to_consume = bytes_needed.min(data.len());

                // append data to body
                self.body.extend_from_slice(&data[..bytes_to_consume]);

                if self.body.len() > content_length {
                    return Err(ParseError::InvalidFormat("body longer than content-length".to_string()));
                } else if self.body.len() == content_length {
                    self.parser_state = ParserState::Done;
                }

                Ok(bytes_to_consume)
            },
            ParserState::Done => {
                Err(ParseError::InvalidFormat("attempting to parse in a done state".to_string()))
            }
        }

    }

    pub fn parse(&mut self, data: &[u8]) -> Result<usize, ParseError> {
        let mut total_bytes_parsed = 0;

        while self.parser_state != ParserState::Done && total_bytes_parsed < data.len() {
            let remaining_data = &data[total_bytes_parsed..];
            let bytes_read = self.parse_single(remaining_data)?;

            if bytes_read == 0 {
                break;
            }

            total_bytes_parsed += bytes_read;
        }
        
        Ok(total_bytes_parsed)
    }
}

#[derive(Debug, PartialEq, Eq, Default)]
pub struct RequestLine {
    http_version: String,
    request_target: String,
    method: String,
}

impl std::fmt::Display for RequestLine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Request line:\n- Method: {}\n- Target: {}\n- Version: {}",
            self.method, self.request_target, self.http_version)
    }
}

impl TryFrom<&str> for RequestLine {
    type Error = ParseError;

    fn try_from(line: &str) -> Result<Self, Self::Error> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        match parts.as_slice() {
            [method, target, version] => {
                for char in method.chars() {
                    if !char.is_alphabetic() {
                        return Err(ParseError::InvalidFormat("method contains non-alphabetic characters".to_string()));
                    }
                }
                match version.split('/').collect::<Vec<_>>().as_slice() {
                    [http_part, version] => {
                        if http_part != &"HTTP" {
                            return Err(ParseError::InvalidFormat("unrecognized protocol".to_string()));
                        }
                        if version != &"1.1" {
                            return Err(ParseError::InvalidFormat("unrecognized http version".to_string()));
                        }
                        Ok(RequestLine {
                            http_version: version.to_string(),
                            request_target: target.to_string(),
                            method: method.to_string(),
                        })
                    },
                    _ => Err(ParseError::InvalidFormat("malformed request line".to_string())),
                }
            },
            _ => Err(ParseError::InvalidFormat("malformed request line".to_string())),
        }
    }
}

impl RequestLine {
    pub fn build(http_version: &str, request_target: &str, method: &str) -> Self {
        RequestLine {
            http_version: http_version.to_string(),
            request_target: request_target.to_string(),
            method: method.to_string(),
        }
    }

    pub fn parse(data: &[u8]) -> Result<(Option<RequestLine>,usize), ParseError> {
        if let Some(idx) = data.windows(2).position(|window| window == b"\r\n") {
            let request_line_text = String::from_utf8_lossy(&data[..idx]);
            let request_line = RequestLine::try_from(request_line_text.as_ref())?;
            Ok((Some(request_line), idx+2))
        } else {
            Ok((None, 0))
        }
    }
}

const BUFFER_SIZE: usize = 8;

pub async fn request_from_reader<R>(mut reader: R) -> Result<Request, ParseError>
    where R: AsyncRead + Unpin
{
    let mut buf = vec![0u8; BUFFER_SIZE];
    let mut read_to_index = 0;
    let mut request = Request::new();

    while request.parser_state != ParserState::Done {
        // grow the buffer as required
        if read_to_index >= buf.len() {
            buf.resize(buf.len()*2, 0);
        }

        // read from the reader into the buffer
        let bytes_read = reader.read(&mut buf[read_to_index..]).await
            .map_err(|_| ParseError::IOError)?;

        if bytes_read == 0 {
            break;
        }
        read_to_index += bytes_read;

        // parse what we have thus far
        let num_bytes_parsed = request.parse(&buf[..read_to_index])?;

        println!("Current buffer state: {:?}", buf);
        println!("   As a string, that's: {}", String::from_utf8_lossy(&buf));

        // slide the buffer left to remove the parsed bytes
        if num_bytes_parsed > 0 {
            buf.copy_within(num_bytes_parsed..read_to_index, 0);
            read_to_index -= num_bytes_parsed;
        }

    }
    Ok(request)
}

#[cfg(test)]
mod test {
    use super::*;
    use std::io::prelude::*;
    use std::io::Cursor;

    #[test]
    fn test_request_line_parse() {
        let line = "GET / HTTP/1.1";
        let rl = RequestLine::try_from(line).unwrap();
        println!("{:?}", rl);
        let expected = RequestLine {
            http_version: "1.1".to_string(),
            request_target: "/".to_string(),
            method: "GET".to_string(),
        };
        assert_eq!(expected, rl);
    }

    #[tokio::test]
    async fn test_request_from_reader() {
        let http_data = "GET /coffee HTTP/1.1\r\nHost: localhost:42069\r\nUser-Agent: curl/7.81.0\r\nAccept: */*\r\n\r\n";
        let reader = Cursor::new(http_data);

        let request = request_from_reader(reader).await.unwrap();

        assert_eq!(request.request_line.method, "GET");
        assert_eq!(request.request_line.request_target, "/coffee");
        assert_eq!(request.request_line.http_version, "1.1");
    }

    #[tokio::test]
    async fn test_invalid_request() {
        let http_data = "/coffee HTTP/1.1\r\nHost: localhost:42069\r\nUser-Agent: curl/7.81.0\r\nAccept: */*\r\n\r\n";
        let reader = Cursor::new(http_data);

        let request = request_from_reader(reader).await;

        assert_eq!(request.err(), Some(ParseError::InvalidFormat("malformed request line".to_string())));
    }

    #[test]
    fn test_standard_headers() {
        let mut request = Request::new();
        let data = b"GET / HTTP/1.1\r\nHost: localhost:42069\r\nUser-Agent: curl/7.81.0\r\nAccept: */*\r\n\r\n";
        
        let _consumed = request.parse(data).unwrap();
        assert_eq!(request.parser_state, ParserState::Done);
        
        // Check request line
        let rl = request.request_line;
        assert_eq!(rl.method, "GET");
        
        // Check headers
        assert_eq!("localhost:42069", request.headers.get("host").unwrap());
        assert_eq!("curl/7.81.0", request.headers.get("user-agent").unwrap());
        assert_eq!("*/*", request.headers.get("accept").unwrap());
    }

    #[test]
    fn test_empty_headers() {
        let mut request = Request::new();
        let data = b"GET / HTTP/1.1\r\n\r\n";  // No headers, just \r\n\r\n
        
        let _consumed = request.parse(data).unwrap();
        assert_eq!(request.parser_state, ParserState::Done);
        assert!(request.headers.is_empty());
    }

    #[test]
    fn test_malformed_header() {
        let mut request = Request::new();
        let data = b"GET / HTTP/1.1\r\nHost localhost:42069\r\n\r\n";  // Missing colon
        
        let result = request.parse(data);
        assert!(result.is_err());
    }

    #[test]
    fn test_standard_body() {
        let mut request = Request::new();
        let data = b"POST /submit HTTP/1.1\r\nHost: localhost:42069\r\nContent-Length: 13\r\n\r\nhello world!\n";
        
        let _consumed = request.parse(data).unwrap();
        assert_eq!(request.parser_state, ParserState::Done);
        assert_eq!("hello world!\n", String::from_utf8_lossy(&request.body));
    }

    #[test]
    fn test_empty_body_zero_content_length() {
        let mut request = Request::new();
        let data = b"GET / HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\n\r\n";
        
        let _consumed = request.parse(data).unwrap();
        assert_eq!(request.parser_state, ParserState::Done);
        assert!(request.body.is_empty());
    }

    #[test]
    fn test_empty_body_no_content_length() {
        let mut request = Request::new();
        let data = b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n";
        
        let _consumed = request.parse(data).unwrap();
        assert_eq!(request.parser_state, ParserState::Done);
        assert!(request.body.is_empty());
    }

    #[test]
    fn test_body_shorter_than_content_length() {
        let mut request = Request::new();
        let data = b"POST /submit HTTP/1.1\r\nHost: localhost\r\nContent-Length: 20\r\n\r\npartial content";
        
        // This should not error immediately - it should wait for more data
        // The error would come from request_from_reader when it hits EOF
        let _consumed = request.parse(data).unwrap();
        assert_eq!(request.parser_state, ParserState::ParsingBody); // Still waiting for more data
        assert_eq!("partial content", String::from_utf8_lossy(&request.body));
    }

    #[test]
    fn test_no_content_length_but_body_exists() {
        let mut request = Request::new();
        let data = b"POST /submit HTTP/1.1\r\nHost: localhost\r\n\r\nsome body data";
        
        let _consumed = request.parse(data).unwrap();
        assert_eq!(request.parser_state, ParserState::Done);
        assert!(request.body.is_empty()); // Body ignored without Content-Length
    }

    // ---------------------------------------

    #[test]
    fn test_duplicate_headers() {
        let mut request = Request::new();
        let data = b"GET / HTTP/1.1\r\nSet-Person: lane\r\nSet-Person: prime\r\n\r\n";
        
        let _consumed = request.parse(data).unwrap();
        assert_eq!("lane, prime", request.headers.get("set-person").unwrap());
    }

    #[test]
    fn test_stateful_request_parsing() {
        let mut request = Request::new();
        
        // Chunk 1: incomplete
        let consumed = request.parse(b"GE").unwrap();
        assert_eq!(consumed, 0); // Not enough data
        assert_eq!(request.parser_state, ParserState::Initialized);
        
        // Chunk 2: complete line
        let consumed = request.parse(b"GET / HTTP/1.1\r\nHost: localhost").unwrap();
        assert_eq!(consumed, 16); // "GET / HTTP/1.1\r\n" = 14 + 2
        assert_eq!(request.parser_state, ParserState::ParsingHeaders);
        assert_eq!(request.request_line.method, "GET");
        assert_eq!(request.request_line.request_target, "/");
        assert_eq!(request.request_line.http_version, "1.1");
    }

    pub struct ChunkReader {
        data: Vec<u8>,
        num_bytes_per_read: usize,
        pos: usize,
    }
    impl ChunkReader {
        fn new(data: &str, bytes_per_read: usize) -> Self {
            ChunkReader {
                data: data.as_bytes().to_vec(),
                num_bytes_per_read: bytes_per_read,
                pos: 0
            }
        }
    }
    impl std::io::Read for ChunkReader {
        
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            if self.pos >= self.data.len() {
                return Ok(0); // EOF
            }
            
            let end_index = (self.pos + self.num_bytes_per_read)
                .min(self.data.len());
            
            let bytes_to_copy = (end_index - self.pos).min(buf.len());

            buf[..bytes_to_copy].copy_from_slice(&self.data[self.pos..self.pos + bytes_to_copy]);
            
            self.pos += bytes_to_copy;
            Ok(bytes_to_copy)
        }
    }

    #[test]
    fn test_chunk_reader_basics() {
        let http_data = "GET /coffee HTTP/1.1\r\nHost: localhost:42069\r\nUser-Agent: curl/7.81.0\r\nAccept: */*\r\n\r\n";
        let mut reader = ChunkReader::new(http_data, 3);
        let mut buf = [0u8; 10];

        let n = reader.read(&mut buf).unwrap();
        assert_eq!(n, 3);
        assert_eq!(&buf[..n], b"GET");

        let n = reader.read(&mut buf).unwrap();
        assert_eq!(n, 3);
        assert_eq!(&buf[..n], b" /c");

    }
}