use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use tokio::{io::{AsyncBufReadExt, AsyncReadExt, BufReader, BufWriter}, net::TcpStream};

use crate::http::{req::{Request, Response}, router::Router};


// ------------------ HTTP Handler ------------------
#[derive(Clone)]
pub struct HTTPHandler {
    router: Arc<Router>,
}

impl HTTPHandler {
    pub fn new(router: Router) -> Self {
        Self { router: Arc::new(router) }
    }

    /// 处理单个 TcpStream（不 bind），只解析 HTTP 并执行路由
    pub async fn handle_stream(&self, mut stream: TcpStream, peer: SocketAddr) {
        let (reader, writer) = stream.split();
        let mut reader = BufReader::new(reader);
        let mut writer = BufWriter::new(writer);

        // 读取请求行
        let mut first_line = String::new();
        if reader.read_line(&mut first_line).await.is_err() { return; }

        let parts: Vec<&str> = first_line.trim().split_whitespace().collect();
        if parts.len() < 2 { return; }
        let method = parts[0];
        let path = parts[1];

        // 读取 headers
        let mut headers = HashMap::new();
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).await.unwrap();
            if line == "\r\n" || line.is_empty() { break; }
            if let Some((k, v)) = line.trim().split_once(": ") {
                headers.insert(k.to_string(), v.to_string());
            }
        }

        // 读取 body
        let length = headers.get("Content-Length")
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(0);
        let mut body = vec![0u8; length];
        reader.read_exact(&mut body).await.unwrap();

        let req = Request {
            method: method.to_string(),
            path: path.to_string(),
            headers,
            body,
            peer_addr: peer,
        };
        let mut res = Response::new(writer);

        if let Some(handler) = self.router.get_handler(&req.path) {
            if let Some(exec) = handler.handle(&req.method) {
                exec(&req, &mut res);
            } else {
                res.send(405, "Method Not Allowed").await;
            }
        } else {
            res.send(404, "Not Found").await;
        }
    }
}