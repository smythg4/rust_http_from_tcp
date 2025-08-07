use rust_http_from_tcp::http::request::request_from_reader;
use tokio::net::TcpListener;


#[tokio::main]
async fn main() -> std::io::Result<()>{
    let listener = TcpListener::bind("127.0.0.1:42069").await?;
    println!("Server listening on port 42069");

    loop {
        let (stream, addr) = listener.accept().await?;
        println!("Accepted connection from: {addr}");

        let request = request_from_reader(stream).await.unwrap();
        
        println!("{request}");
        
        println!("Connection closed.");
    }
}
