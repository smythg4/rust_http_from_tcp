use tokio::io::{AsyncReadExt, BufReader};
use tokio::sync::mpsc;
use tokio::net::{TcpListener, TcpStream};

fn get_lines_channel(stream: TcpStream) -> mpsc::Receiver<String> {
    let (tx, rx) = mpsc::channel::<String>(32);

    tokio::spawn(async move {
        let mut reader = BufReader::new(stream);
        let mut buf = [0; 8];

        let mut string_so_far = String::new();

        while let Ok(n) = reader.read(&mut buf).await {
            if n == 0 { break; }  // EOF reached
            let s = String::from_utf8_lossy(&buf[..n]);
            let parts: Vec<_> = s.split('\n').collect();
            
            match parts.as_slice() {
                [] => {
                    unreachable!("split always returns at least one part")
                }
                [single_part] => {
                    string_so_far.push_str(&single_part);
                },
                [first, rest @ ..] => {
                    string_so_far.push_str(&first);
                    if tx.send(string_so_far.clone()).await.is_err(){
                        break; // receiver dropped
                    }
                    string_so_far.clear();

                    for part in &rest[..rest.len()-1] {
                        if tx.send(part.to_string()).await.is_err() {
                            return;
                        }
                    }

                    string_so_far.push_str(rest.last().unwrap());
                },
            }
        }
        if !string_so_far.is_empty() {
            let _ = tx.send(string_so_far).await;
        }
    });

    rx
}

#[tokio::main]
async fn main() -> std::io::Result<()>{
    let listener = TcpListener::bind("127.0.0.1:42069").await?;
    println!("Server listening on port 42069");

    loop {
        let (stream, addr) = listener.accept().await?;
        println!("Accepted connection from: {addr}");

        let mut rx = get_lines_channel(stream);
        
        while let Some(line) = rx.recv().await {
            println!("{line}");
        }
        
        println!("Connection closed.");
    }

    Ok(())
}
