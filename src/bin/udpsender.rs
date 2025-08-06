use tokio::net::UdpSocket;
use tokio::io::{self, AsyncBufReadExt, BufReader};
use std::io::Write;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    println!("UDP sender starting...");

    let socket = UdpSocket::bind("0.0.0.0:0").await?;

    socket.connect("localhost:42069").await?;
    println!("Ready to send to localhost:42069");

    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin);
    let mut line = String::new();

    loop {
        print!("> ");
        std::io::stdout().flush()?;

        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) => break, // EOF reached
            Ok(_) => {
                match socket.send(line.as_bytes()).await {
                    Ok(bytes_sent) => {
                        println!("Sent {} bytes", bytes_sent);
                    },
                    Err(e) => {
                        eprintln!("Error sending: {}", e);
                    }
                }
            },
            Err(e) => eprintln!("Error reading input: {}", e),
        }
    }
    Ok(())
}