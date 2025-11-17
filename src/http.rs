use tokio::net::TcpStream;
use tokio::io::{ AsyncReadExt, AsyncWriteExt };

use crate::share::HTTP_BUFFER_LENGTH;

pub struct HTTPHandler {
    ip: String,
    port: u16,
    stream: TcpStream,
}

impl HTTPHandler {
    pub fn new(ip: &String, port: u16, stream: TcpStream) -> Self {
        HTTPHandler { ip: ip.clone(), port, stream }
    }

    pub async fn start(self) {
        tokio::spawn(async move {
            let mut stream = self.stream;
            loop {
                let mut buf = vec![0u8; HTTP_BUFFER_LENGTH];
                match stream.read(&mut buf).await {
                    Ok(0) => {
                        break;
                    } // 连接关闭
                    Ok(n) => {
                        println!("HTTP received {} bytes: {:?}", n, &buf[..n]);
                        let _ = stream.write_all(&buf[..n]).await;
                    }
                    Err(e) => {
                        eprintln!("HTTP read error: {:?}", e);
                        break;
                    }
                }
            }
        });
    }
}
