use tokio::net::{TcpListener, TcpStream};
use tokio::signal;
use tokio_util::io::StreamReader;
use tokio::io::AsyncReadExt;

use futures_util::StreamExt;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tokio::fs::File;

use sha2::{Sha256, Digest};

use rust_http_from_tcp::http::response::{Response, StatusCode, Writer};
use rust_http_from_tcp::http::request::{request_from_reader, Request};
use rust_http_from_tcp::http::headers::Headers;

const PORT: u16 = 42069;

//type Handler = Arc<dyn Fn(&mut Writer, &Request) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), ServerError>> + Send + '_>> + Send + Sync>;

#[derive(Debug)]
pub enum ServerError{
    BindError(std::io::Error),
    ConnectionError(std::io::Error),
    ReqwestError(reqwest::Error),
    HandlerError { status_code: StatusCode, message: String },
}

impl std::fmt::Display for ServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServerError::BindError(e) => write!(f,"Failed to bind to address: {}", e),
            ServerError::ConnectionError(e) => write!(f, "Connection error: {}", e),
            ServerError::HandlerError{status_code, message} => write!(f, "Handler error: {} - {}", status_code, message),
            ServerError::ReqwestError(e) => write!(f, "Reqwest fetch error: {}", e),
        }
    }
}

impl std::error::Error for ServerError {}

impl From<std::io::Error> for ServerError {
    fn from(error: std::io::Error) -> Self {
        ServerError::ConnectionError(error)
    }
}

impl ServerError {
    pub fn bad_request(message: &str) -> Self {
        ServerError::HandlerError {
            status_code: StatusCode::StatusBadRequest,
            message: message.to_string(),
        }
    }

    pub fn internal_error(message: &str) -> Self {
        ServerError::HandlerError {
            status_code: StatusCode::StatusInternalServerError,
            message: message.to_string(),
        }
    }
}

pub struct Server {
    listener: Arc<TcpListener>,
    is_closed: Arc<AtomicBool>,
    //handler: Arc<Handler>,
}

impl Server {

    pub async fn serve(port: u16) -> Result<Server, ServerError> {//, handler: Handler) -> Result<Server, ServerError> {
        let addr = format!("127.0.0.1:{}", port);
        let listener = Arc::new(TcpListener::bind(&addr).await
            .map_err(ServerError::BindError)?);
        let is_closed = Arc::new(AtomicBool::new(false));

        let server = Server {
            listener: listener.clone(),
            is_closed: is_closed.clone(),
            //handler: Arc::new(handler).clone(),
        };

        server.start_listening();

        Ok(server)
    }

    fn start_listening(&self) {
        let listener = self.listener.clone();
        let is_closed = self.is_closed.clone();
        //let handler = self.handler.clone();

        tokio::spawn(async move {
            Self::listen_loop(listener, is_closed).await;//, handler).await;
        });
    }

    async fn listen_loop(listener: Arc<TcpListener>, is_closed: Arc<AtomicBool>){//}, handler: Arc<Handler>) {
        loop {
            if is_closed.load(Ordering::Relaxed) {
                break;
            }

            match listener.accept().await {
                Ok((stream, addr)) => {
                    println!("Accepted connection from: {}", addr);
                    //let handler = handler.clone();
                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_connection(stream).await {//}, handler).await {
                            eprintln!("Error handling connection: {}", e);
                        }
                    });
                },
                Err(_) => break,
            }
        }
    }

    async fn handle_connection(mut stream: TcpStream) -> Result<(), ServerError> {//, handler: Arc<Handler>) -> Result<(), ServerError> {
        let request = request_from_reader(&mut stream).await
            .map_err(|e| ServerError::internal_error(e.to_string().as_str()))?;
        let mut writer = Writer::new(stream);
        my_handler(&mut writer, &request).await?;
        Ok(())
    }

    pub async fn close(self) -> Result<(), ServerError> {
        self.is_closed.store(true, Ordering::Relaxed);
        Ok(())
    }
}


async fn handle_400(writer: &mut Writer) -> Result<(), ServerError> {
    let html = r#"<html>
<head>
    <title>400 Bad Request</title>
</head>
<body>
    <h1>Bad Request</h1>
    <p>Your request honestly kinda sucked.</p>
</body>
</html>"#;
    writer.write_status_line(StatusCode::StatusBadRequest).await
        .map_err(ServerError::ConnectionError)?;

    let mut headers = Response::get_default_headers(html.len());
    headers.insert("content-type".to_string(), "text/html".to_string());
    writer.write_headers(&headers).await
        .map_err(ServerError::ConnectionError)?;

    writer.write_body(html.as_bytes()).await
        .map_err(ServerError::ConnectionError)?;
    
    Ok(())
}

async fn handle_500(writer: &mut Writer) -> Result<(), ServerError> {
    
        let html = r#"<html>
<head>
<title>500 Internal Server Error</title>
</head>
<body>
<h1>Internal Server Error</h1>
<p>Okay, you know what? This one is on me.</p>
</body>
</html>"#;
        writer.write_status_line(StatusCode::StatusInternalServerError).await
            .map_err(ServerError::ConnectionError)?;

        let mut headers = Response::get_default_headers(html.len());
        headers.insert("content-type".to_string(), "text/html".to_string());
        writer.write_headers(&headers).await
            .map_err(ServerError::ConnectionError)?;

        writer.write_body(html.as_bytes()).await
            .map_err(ServerError::ConnectionError)?;
        Ok(())
}

async fn handle_200(writer: &mut Writer) -> Result<(), ServerError> {
    let html = r#"<html>
<head>
<title>200 OK</title>
</head>
<body>
<h1>Success!</h1>
<p>Your request was an absolute banger!</p>
</body>
</html>"#;
    writer.write_status_line(StatusCode::StatusOk).await
        .map_err(ServerError::ConnectionError)?;

    let mut headers = Response::get_default_headers(html.len());
    headers.insert("content-type".to_string(), "text/html".to_string());
    writer.write_headers(&headers).await
        .map_err(ServerError::ConnectionError)?;

    writer.write_body(html.as_bytes()).await
        .map_err(ServerError::ConnectionError)?;

    Ok(())
}

async fn handle_httpbin(httpbin: &str, writer: &mut Writer) -> Result<(), ServerError> {
    let endpoint = httpbin.trim_start_matches("/httpbin/");
    let full_url = format!("https://httpbin.org/{}", endpoint);
    let get_response = reqwest::get(full_url)
        .await
        .map_err(ServerError::ReqwestError)?;

    writer.write_status_line(StatusCode::StatusOk).await?;

    let mut headers = Response::get_default_headers(0);
    headers.remove_entry("Content-Length");
    headers.insert("Transfer-Encoding".to_string(), "chunked".to_string());
    headers.insert("trailer".to_string(), "X-Content-SHA256, X-Content-Length".to_string());

    writer.write_headers(&headers).await?;

    let mut full_body = Vec::new();
    let mut total_bytes = 0usize;

    let stream = get_response.bytes_stream().map(|result| {
        result.map_err(|e| std::io::Error::new(
            std::io::ErrorKind::Other, e))
    });

    let mut reader = StreamReader::new(stream);

    const CHUNK_SIZE: usize = 32;
    let mut buffer = [0u8; CHUNK_SIZE];

    loop {
        let bytes_read = reader.read(&mut buffer).await
            .map_err(ServerError::ConnectionError)?;

        if bytes_read == 0 {
            break; // EOF reached
        }

        let chunk = &buffer[..bytes_read];

        full_body.extend_from_slice(chunk);
        total_bytes += bytes_read;

        println!("Read {} bytes from the stream", bytes_read);
        writer.write_chunked_body(chunk).await?;
    }
    writer.write_chunked_body_done().await?;

    let mut hasher = Sha256::new();
    hasher.update(&full_body);
    let hash = hasher.finalize();
    let hash_hex = hex::encode(hash);

    //write trailers
    let mut trailer_headers = Headers::new();
    trailer_headers.insert("X-Content-SHA256".to_string(), hash_hex);
    trailer_headers.insert("X-Content-Length".to_string(), total_bytes.to_string());

    writer.write_trailers(&trailer_headers).await?;
    writer.finish().await?;

    Ok(())
}

async fn handle_video(writer: &mut Writer) -> Result<(), ServerError> {
    let f = File::open("assets/vim.mp4").await;

    let mut f = match f {
        Ok(f) => f,
        Err(_) => {
            //file not found!
            let message = b"Video not found :(";
            writer.write_status_line(StatusCode::StatusNotFound).await?;
            let headers = Response::get_default_headers(message.len());
            writer.write_headers(&headers).await?;
            writer.write_body(message).await?;
            return Ok(());
        }
    };

    writer.write_status_line(StatusCode::StatusOk).await?;

    let mut headers = Response::get_default_headers(0);
    headers.remove_entry("Content-Length");
    headers.insert("transfer-encoding".to_string(), "chunked".to_string());
    headers.insert("content-type".to_string(), "video/mp4".to_string());
    writer.write_headers(&headers).await?;

    const CHUNK_SIZE: usize = 1024;
    let mut buffer = [0u8; CHUNK_SIZE];

    loop {
        let bytes_read = f.read(&mut buffer).await?;
        if bytes_read == 0{
            break; // EOF
        }

        writer.write_chunked_body(&buffer[..bytes_read]).await?;
    }

    writer.write_chunked_body_done().await?;
    writer.finish().await?;

    Ok(())
}

async fn my_handler(mut writer: &mut Writer, req: &Request) -> Result<(), ServerError> {
    match req.get_target() {
        httpbin if httpbin.starts_with("/httpbin/") => handle_httpbin(httpbin, &mut writer).await?,
        "/video" => handle_video(&mut writer).await?,
        "/yourproblem" => handle_400(&mut writer).await?,
        "/myproblem" => handle_500(&mut writer).await?,
        _ => handle_200(&mut writer).await?,
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    let server = Server::serve(PORT).await?;
    println!("Server started on port {}", PORT);

    signal::ctrl_c().await?;
    println!("Shutting down server...");

    server.close().await?;
    println!("Server gracefully stopped.");

    Ok(())
}