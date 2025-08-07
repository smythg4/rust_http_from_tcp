use tokio::net::{TcpListener, TcpStream};
use tokio::signal;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use rust_http_from_tcp::http::response::{Response, StatusCode, Writer};
use rust_http_from_tcp::http::request::{request_from_reader, Request};

const PORT: u16 = 42069;

//type Handler = fn(&mut Writer, &Request) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), ServerError>> + Send>>;

#[derive(Debug)]
pub enum ServerError{
    BindError(std::io::Error),
    ConnectionError(std::io::Error),
    HandlerError { status_code: StatusCode, message: String },
}

impl std::fmt::Display for ServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServerError::BindError(e) => write!(f,"Failed to bind to address: {}", e),
            ServerError::ConnectionError(e) => write!(f, "Connection error: {}", e),
            ServerError::HandlerError{status_code, message} => write!(f, "Handler error: {} - {}", status_code, message),
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
    //handler: Handler,
}

impl Server {

    pub async fn serve(port: u16) -> Result<Server, ServerError> {
        let addr = format!("127.0.0.1:{}", port);
        let listener = Arc::new(TcpListener::bind(&addr).await
            .map_err(ServerError::BindError)?);
        let is_closed = Arc::new(AtomicBool::new(false));

        let server = Server {
            listener: listener.clone(),
            is_closed: is_closed.clone(),
            //handler,
        };

        server.start_listening();

        Ok(server)
    }

    fn start_listening(&self) {
        let listener = self.listener.clone();
        let is_closed = self.is_closed.clone();
        //let handler = self.handler;

        tokio::spawn(async move {
            Self::listen_loop(listener, is_closed).await;
        });
    }

    async fn listen_loop(listener: Arc<TcpListener>, is_closed: Arc<AtomicBool>) {
        loop {
            if is_closed.load(Ordering::Relaxed) {
                break;
            }

            match listener.accept().await {
                Ok((stream, addr)) => {
                    println!("Accepted connection from: {}", addr);
                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_connection(stream).await {
                            eprintln!("Error handling connection: {}", e);
                        }
                    });
                },
                Err(_) => break,
            }
        }
    }

    async fn handle_connection(mut stream: TcpStream) -> Result<(), ServerError> {
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

async fn my_handler(writer: &mut Writer, req: &Request) -> Result<(), ServerError> {
    match req.get_target() {
        "/yourproblem" => {
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
        },
        "/myproblem" => {
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
        },
        _ => {
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
        },
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