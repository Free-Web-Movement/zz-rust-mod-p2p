use std::{ collections::HashMap, net::SocketAddr, path::Path };
use tokio::{ fs::File, io::{ AsyncReadExt, AsyncWriteExt, BufWriter }, net::TcpStream };
use crate::http::protocol::{ header::HeaderKey, media_type::MediaType };
use crate::http::protocol::status::StatusCode;
use serde::Serialize;

const HTTP_WRITE_BUFFER: usize = 8 * 1024 * 1024;

/// HTTP 响应结构
pub struct Response {
    pub writer: BufWriter<tokio::io::WriteHalf<TcpStream>>,
    peer_addr: SocketAddr,
}

impl Response {
    pub fn new(writer: BufWriter<tokio::io::WriteHalf<TcpStream>>, peer_addr: SocketAddr) -> Self {
        Response { writer, peer_addr }
    }
    /// 直接写入字符串（不封装 HTTP 响应）
    pub async fn write_str<S: AsRef<str>>(&mut self, s: S) -> std::io::Result<()> {
        self.writer.write_all(s.as_ref().as_bytes()).await?;
        self.writer.flush().await
    }

    /// 直接写入字节（不封装 HTTP 响应）
    pub async fn write_bytes(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        self.writer.write_all(bytes).await?;
        self.writer.flush().await
    }

    /// 核心发送方法，内部统一处理 header 拼接、Content-Length 和 flush
    async fn send_inner(
        &mut self,
        status: StatusCode,
        mut headers: HashMap<String, String>,
        body: &[u8]
    ) -> std::io::Result<()> {
        // 自动设置 Content-Length（除非已经设置）
        headers
            .entry(HeaderKey::ContentLength.to_str().to_string())
            .or_insert(body.len().to_string());

        // 构造响应头
        let mut response = format!("HTTP/1.1 {} {}\r\n", status as u16, status.to_str());
        for (key, value) in &headers {
            response.push_str(&format!("{}: {}\r\n", key, value));
        }
        response.push_str("\r\n");

        // 写入 header + body
        self.writer.write_all(response.as_bytes()).await?;
        self.writer.write_all(body).await?;
        self.writer.flush().await
    }

    /// 发送任意字节 body
    pub async fn send_bytes(
        &mut self,
        status: StatusCode,
        headers: HashMap<String, String>,
        body: &[u8]
    ) -> std::io::Result<()> {
        self.send_inner(status, headers, body).await
    }

    /// 发送字符串 body
    pub async fn send_str<S: AsRef<str>>(
        &mut self,
        status: StatusCode,
        headers: HashMap<String, String>,
        body: S
    ) -> std::io::Result<()> {
        self.send_inner(status, headers, body.as_ref().as_bytes()).await
    }

    /// 发送 JSON 数据（自动设置 Content-Type: application/json）
    pub async fn send_json<T: Serialize>(
        &mut self,
        status: StatusCode,
        mut headers: HashMap<String, String>,
        value: &T
    ) -> std::io::Result<()> {
        // 序列化 JSON
        let json_body = serde_json
            ::to_vec(value)
            .map_err(|e| {
                std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("JSON serialize error: {}", e)
                )
            })?;

        // 设置 Content-Type: application/json
        headers
            .entry(HeaderKey::ContentType.to_str().to_string())
            .or_insert("application/json".to_string());

        self.send_inner(status, headers, &json_body).await
    }

    /// 只发送状态码，body 为空（Content-Length: 0）
    pub async fn send_status_only(
        &mut self,
        status: StatusCode,
        mut headers: HashMap<String, String>
    ) -> std::io::Result<()> {
        headers.entry(HeaderKey::ContentLength.to_str().to_string()).or_insert("0".to_string());

        self.send_inner(status, headers, &[]).await
    }

    /// 发送本地文件
    pub async fn send_file<P: AsRef<Path>>(
        &mut self,
        status: StatusCode,
        mut headers: HashMap<String, String>,
        file_path: P
    ) -> std::io::Result<()> {
        let path = file_path.as_ref();

        // 打开文件
        let mut file = File::open(path).await?;
        let file_size = file.metadata().await?.len();

        // 设置 Content-Length
        headers
            .entry(HeaderKey::ContentLength.to_str().to_string())
            .or_insert(file_size.to_string());

        // 设置 Content-Type（如果没设置的话）
        headers
            .entry(HeaderKey::ContentType.to_str().to_string())
            .or_insert(MediaType::guess(path).to_string());

        // 发送响应头
        let mut head = format!("HTTP/1.1 {}\r\n", status as usize);
        for (k, v) in &headers {
            head.push_str(&format!("{}: {}\r\n", k, v));
        }
        head.push_str("\r\n");
        self.writer.write_all(head.as_bytes()).await?;

        // 分块读取文件并写入
        let mut buffer = [0u8; HTTP_WRITE_BUFFER]; // 8KB 缓冲
        loop {
            let n = file.read(&mut buffer).await?;
            if n == 0 {
                break; // EOF
            }
            self.writer.write_all(&buffer[..n]).await?;
        }

        self.writer.flush().await
    }
}
