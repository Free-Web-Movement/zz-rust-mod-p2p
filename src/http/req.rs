use futures::executor::block_on;
use tokio::io::{ AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, BufWriter };
use tokio::net::{ TcpStream };

use std::net::SocketAddr;
use std::collections::HashMap;

enum RequestHeadKeys {
    CONTENT_TYPE = 0,
    CONTENT_LENGTH = 1,
}

const REQUEST_HEAD_KEYS: [&'static str; 2] = ["Content-Type", "Content-Length"];

pub struct Request {
    pub method: String,
    pub path: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
    pub reader: BufReader<tokio::io::ReadHalf<TcpStream>>,
    peer_addr: SocketAddr,
}

pub struct Response {
    // pub status: String,
    // pub headers: HashMap<String, String>,
    // pub body: Vec<u8>,
    pub writer: BufWriter<tokio::io::WriteHalf<TcpStream>>,
    peer_addr: SocketAddr,
}

impl Response {
    pub fn new(writer: BufWriter<tokio::io::WriteHalf<TcpStream>>, peer_addr: SocketAddr) -> Self {
        Response {
            // status,
            // headers,
            // body,
            writer,
            peer_addr,
        }
    }

    pub async fn send(&mut self, status: usize, headers: HashMap<String, String>, body: Vec<u8>) {
        let mut response = format!("HTTP/1.1 {}\r\n", status);
        for (key, value) in &headers {
            response.push_str(&format!("{}: {}\r\n", key, value));
        }
        response.push_str("\r\n");
        response.push_str(&String::from_utf8_lossy(&body));

        self.writer.write_all(response.as_bytes()).await.unwrap();
        self.writer.flush().await.unwrap()
    }

    pub async fn write(&mut self, text: &'static str) {
        // println!("inside response write: {}!", text);
        self.writer.write_all(text.as_bytes()).await.unwrap();
        self.writer.flush().await.unwrap()
    }

    pub fn send_bytes(&mut self, bytes: &[u8]) {
        let future = self.writer.write_all(bytes);
        let _ = block_on(future);
    }

    pub fn send_string(&mut self, str: &String) {
        let future = self.writer.write_all(str.as_bytes());
        let _ = block_on(future);
    }
}

impl Request {
    pub async fn new(
        mut reader: BufReader<tokio::io::ReadHalf<TcpStream>>,
        peer_addr: SocketAddr
    ) -> Self {
        let mut status = String::new();
        reader.read_line(&mut status).await.unwrap();
        let parts = status.split_whitespace().collect::<Vec<&str>>();
        assert!(parts.len() >= 2, "Invalid HTTP request line");

        let method = parts[0];
        let path = parts[1];
        println!("Received HTTP request from {}: {} {}", peer_addr, method, path);
        let headers = Request::read_headers(&mut reader).await;
        let length = Request::get_length(&headers);
        println!("Length: {}", length);
        let mut body = vec![0u8; length];
        reader.read_exact(&mut body).await.unwrap();
        Request {
            method: method.to_string(),
            path: path.to_string(),
            headers,
            body,
            reader,
            peer_addr,
        }
    }

    pub fn get_length(headers: &HashMap<String, String>) -> usize {
        headers
            .get(REQUEST_HEAD_KEYS[RequestHeadKeys::CONTENT_TYPE as usize])
            .and_then(|s| s.trim().parse::<usize>().ok())
            .unwrap_or(0)
    }

    pub fn get_method(&self) -> &str {
        &self.method
    }

    pub async fn read_headers(
        reader: &mut BufReader<tokio::io::ReadHalf<TcpStream>>
    ) -> HashMap<String, String> {
        let mut mapped_headers = HashMap::new();
        let mut headers = vec![String::new()];
        let mut buf = String::new();

        loop {
            reader.read_line(&mut buf).await.unwrap();
            if buf != "\r\n" && !buf.is_empty() {
                let map = buf.splitn(2, ": ").collect::<Vec<&str>>();
                if map.len() == 2 {
                    mapped_headers.insert(map[0].to_string(), map[1].to_string());
                }
                headers.push(buf.clone());
                buf.clear();
            } else {
                break;
            }
        }
        mapped_headers
    }
}