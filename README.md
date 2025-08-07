# HTTP Server from Scratch in Rust 🦀

A HTTP/1.1 server built from first principles in Rust, implementing chunked transfer encoding, trailers, and streaming.

This was a boot.dev lesson for implementation in Go. I ported it to Rust as close as I could. I never did figure out generic handlers though...

## ✨ Features

- **Full HTTP/1.1 Protocol Support**
  - Request line parsing (method, path, version validation)
  - RFC-compliant header parsing with case-insensitive keys
  - Request body parsing with Content-Length support
  - Proper HTTP status codes (200, 400, 404, 500)

- **Advanced HTTP Features**
  - Chunked transfer encoding for streaming responses
  - HTTP trailers with SHA-256 content hashing
  - Static file serving with proper MIME types

- **High Performance Architecture**
  - Async/await with Tokio runtime
  - Zero-copy streaming with controlled chunk sizes
  - Memory-safe parsing with automatic buffer management
  - Concurrent connection handling

## 🚀 Quick Start

### Prerequisites

- Rust 1.70+ with Cargo
- Assets directory with `vim.mp4` file (optional, for video serving)

### Installation

```bash
git clone https://github.com/smythg4/rust-http-from-tcp
cd rust-http-from-tcp
cargo build --release
```

### Running the Server

```bash
cargo run --bin httpserver
# Server starts on http://localhost:42069
```

## 🎯 API Endpoints

### Basic Routes
- `GET /` - Returns success page with HTML
- `GET /yourproblem` - Returns 400 Bad Request with custom HTML
- `GET /myproblem` - Returns 500 Internal Server Error with custom HTML

### Advanced Features
- `GET /video` - Streams MP4 video file with proper headers
- `GET /httpbin/*` - Proxies requests to httpbin.org with chunked encoding and trailers

### Testing Examples

```bash
# Basic HTML response
curl http://localhost:42069/

# Test video streaming
curl http://localhost:42069/video --output video.mp4

# Test chunked proxy with raw output
curl --raw http://localhost:42069/httpbin/stream/10

# Test with netcat to see raw HTTP
echo -e "GET /httpbin/stream/5 HTTP/1.1\r\nHost: localhost:42069\r\nConnection: close\r\n\r\n" | nc localhost 42069
```

## 🏗️ Architecture

### Core Components

```
src/
├── bin/
│   └── httpserver.rs          # Main server binary
├── http/
│   ├── mod.rs                 # Module exports
│   ├── request.rs             # HTTP request parsing
│   ├── response.rs            # HTTP response writing
│   └── headers.rs             # Header management
└── lib.rs                     # Library root
```

### Request Processing Flow

1. **TCP Connection** - Accept incoming connections
2. **HTTP Parsing** - Stream-based request parsing
3. **Route Matching** - Pattern matching on request paths  
4. **Response Generation** - Async response writing
5. **Connection Cleanup** - Graceful connection closing

### State Machine Design

The HTTP parser uses a state machine with these states:
- `Initialized` → Parse request line
- `ParsingHeaders` → Parse header fields
- `ParsingBody` → Parse request body (if Content-Length present)
- `Done` → Request fully parsed

## 🔬 Technical Implementation

### Chunked Transfer Encoding

The server implements RFC-compliant chunked encoding:

```
HTTP/1.1 200 OK
Transfer-Encoding: chunked
Trailer: X-Content-SHA256, X-Content-Length

1F4
[492 bytes of data]
1F4
[492 bytes of data]
0

X-Content-SHA256: a1b2c3d4e5f6...
X-Content-Length: 1000

```

### Stream Processing

- **Controlled Chunk Sizes**: 32-byte chunks for debugging, 1024-byte for production
- **Memory Efficient**: Streaming with fixed-size buffers
- **Real-time Processing**: Data forwarded as it arrives

### Error Handling

- **Graceful Degradation**: 404 for missing video files
- **Proper HTTP Status Codes**: Semantically correct responses
- **Connection Safety**: Errors don't crash the server

## ⚡ Performance Characteristics

- **Zero-cost Abstractions**: No runtime overhead from safety guarantees
- **Memory Safety**: No memory leaks or buffer overflows possible
- **Concurrent**: Handles multiple connections simultaneously
- **Streaming**: Constant memory usage regardless of response size

## 🧪 Testing

### Unit Tests
```bash
cargo test
```

### Integration Tests
```bash
# Start server
cargo run --bin httpserver

# Test basic functionality
curl -v http://localhost:42069/

# Test chunked encoding
curl --raw http://localhost:42069/httpbin/json

```

### Stress Testing
```bash
# Use wrk or similar tools
wrk -t12 -c400 -d30s http://localhost:42069/
```

## 📚 Learning Resources

This project implements HTTP/1.1 according to:
- [RFC 9110 - HTTP Semantics](https://tools.ietf.org/rfc/rfc9110.txt)
- [RFC 9112 - HTTP/1.1](https://tools.ietf.org/rfc/rfc9112.txt)

Key learning topics covered:
- HTTP protocol internals
- Rust async programming
- Network protocol implementation
- Streaming data processing
- Memory-safe systems programming

## 🔧 Dependencies

```toml
[dependencies]
tokio = { version = "1.0", features = ["full"] }
tokio-util = "0.7"
futures-util = { version = "0.3", features = ["stream"] }
reqwest = { version = "0.12", features = ["stream"] }
sha2 = "0.10"
hex = "0.4"
```

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -am 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## 🙏 Acknowledgments

- Built following Boot.dev's HTTP protocol course
- Inspired by the elegance of Go's net/http package
- Implemented with Rust's safety and performance guarantees

---

**Built with ❤️ and lots of ☕ in Rust** 🦀