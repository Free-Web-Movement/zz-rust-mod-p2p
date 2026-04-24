use aex::{
    connection::{
        entry::ConnectionEntry, global::GlobalContext,
        heartbeat::HeartbeatConfig, scope::NetworkScope,
    },
    crypto::session_key_manager::PairedSessionKey,
    server::{HTTPServer, Server},
    storage::Storage,
    tcp::router::Router as TcpRouter,
};
use chrono::Utc;
use futures::future::FutureExt;
use std::{collections::HashSet, net::SocketAddr, sync::Arc};
use tokio::io::{AsyncReadExt, BufReader, BufWriter};
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use std::io::Cursor;
use zz_account::address::FreeWebMovementAddress;

use crate::{
    cli::{Cli, Opt},
    io_storage::{
        IOStorage, STORAGE_ADDRESS, STORAGE_EXTERNAL_SERVER, STORAGE_INNER_SERVER, io_stroage_init,
    },
    protocols::{command::P2PCommand, frame::P2PFrame, registry::register},
    record::{self, NodeRecord, NodeRegistry},
};

pub type WebHandler = Arc<dyn Fn(&mut aex::connection::context::Context) -> futures::future::BoxFuture<'static, bool> + Send + Sync>;

// 用于保存节点的所有信息
// 用于当前程序的基本信息共享

#[derive(Clone)]
pub struct Node {
    pub id: FreeWebMovementAddress,
    pub inner: NodeRegistry,
    pub external: NodeRegistry,
    // pub storage: Arc<Storage>,
    pub io_storage: IOStorage, // Unique network address of the node, also id for the node
    pub name: String,          // User defined name for the node, no need to be unique
    pub addr: SocketAddr,
    pub context: Arc<GlobalContext>, // global context
    pub server: Server,
    pub cli: Arc<Cli>,
}

impl Node {
    pub async fn new(
        name: String,
        // storage: Arc<Storage>,
        io_storage: IOStorage,
        addr: SocketAddr,
        context: Arc<GlobalContext>,
        server: Server,
        cli: Arc<Cli>,
    ) -> Self {
        let id = io_storage
            .read::<FreeWebMovementAddress>(STORAGE_ADDRESS)
            .await
            .unwrap();
        let inner_nodes = io_storage
            .read::<HashSet<NodeRecord>>(STORAGE_INNER_SERVER)
            .await
            .unwrap();
        let external_nodes = io_storage
            .read::<HashSet<NodeRecord>>(STORAGE_EXTERNAL_SERVER)
            .await
            .unwrap();
        let inner = NodeRegistry::new(inner_nodes);
        let external = NodeRegistry::new(external_nodes);
        Self {
            name,
            id,
            inner,
            external,
            io_storage,
            addr,
            context,
            server,
            cli,
        }
    }

    pub async fn connect(&mut self) {
        let manager = self.context.manager.clone();
        let global = self.context.clone();
        
        let nodes: Vec<record::NodeRecord> = self.inner.nodes.iter().cloned().collect();

        for record in nodes {
            let endpoint = record.endpoint;
            let g = global.clone();

            let _ = manager
                .connect::<P2PFrame, P2PCommand, _, _>(
                    endpoint,
                    g,
                    move |_ctx| {
                        Box::pin(async move {
                            // Connection handler will be dispatched through router
                        })
                    },
                    Some(10),
                )
                .await;
        }
    }

    pub async fn stop(&mut self) {}

    pub async fn init(opt: Opt) -> Self {
        let storage = Arc::new(Storage::new(opt.data_dir.as_deref()));
        let io_storage = io_stroage_init(&opt, storage.clone());

        let addr = format!("{}:{}", opt.ip.clone(), opt.port)
            .parse::<SocketAddr>()
            .unwrap();
        let psk = Arc::new(Mutex::new(PairedSessionKey::new(16)));
        
        let heartbeat_config = HeartbeatConfig::new()
            .with_interval(30)
            .with_timeout(10)
            .on_timeout(|peer_addr| {
                tracing::warn!("Connection timeout: {}", peer_addr);
            })
            .on_latency(|peer_addr, latency| {
                tracing::debug!("Latency for {}: {}ms", peer_addr, latency);
            });
        
        let mut global = GlobalContext::new(addr, Some(psk));
        global.heartbeat_config = heartbeat_config.clone();
        
        let global = Arc::new(global);

        let address: FreeWebMovementAddress = io_storage
            .read::<FreeWebMovementAddress>(&STORAGE_ADDRESS)
            .await
            .unwrap();

        global.set(address.clone()).await;

        let address_1 = global.get::<FreeWebMovementAddress>().await.unwrap();
        assert_eq!(address.to_string(), address_1.to_string());
        global.set(storage.clone()).await;
        global.set(io_storage.clone()).await;
        let cli = Cli::new();

        let server = HTTPServer::new(addr, Some(global.clone()));

        let router = TcpRouter::<P2PFrame, P2PCommand>::new();

        let router = register(router);
        let server = server.tcp(router);
        
        Node::new(
            opt.name,
            io_storage,
            addr,
            global.clone(),
            server,
            Arc::new(cli),
        )
        .await
    }

    pub async fn start<R>(&mut self, reader: R)
    where
        R: tokio::io::AsyncBufRead + Unpin,
    {
        // 1. 克隆需要的资源
        let server = self.server.clone();
        let cli = self.cli.clone();
        let ctx = self.context.clone();

        // 2. 启动 Server (后台运行)
        // 使用 tokio::spawn 确保 server 不会阻塞主线程对 CLI 的处理
        let server_handle = tokio::spawn(async move {
            if let Err(e) = server.start_with_protocols::<P2PFrame, P2PCommand>().await {
                eprintln!("Server error: {:?}", e);
            }
        });

        // 3. 启动 CLI (前台运行)
        // CLI 的退出（输入 exit）将决定 start 函数的结束
        tracing::info!("CLI started. Type 'help' for commands.");
        let _ = cli.run(reader, ctx).await;

        // 4. (可选) 当 CLI 退出后，可以尝试关闭或等待 server
        server_handle.abort(); // 如果希望立即停止 server
    }

    pub async fn start_with_web<R>(self, reader: R, web_handler: WebHandler)
    where
        R: tokio::io::AsyncBufRead + Unpin,
    {
        let server = self.server.clone();
        let cli = self.cli.clone();
        let ctx = self.context.clone();
        let addr = self.addr;
        let manager = self.context.manager.clone();
        let global = self.context.clone();
        let http_handler = web_handler.clone();

        let http_handle = tokio::spawn(async move {
            let listener = match TcpListener::bind(addr).await {
                Ok(l) => l,
                Err(e) => {
                    tracing::error!("Failed to bind: {}", e);
                    return;
                }
            };
            tracing::info!("Unified server started on {}", addr);

            loop {
                tokio::select! {
                    result = listener.accept() => {
                        let (socket, peer_addr) = match result {
                            Ok(r) => r,
                            Err(e) => {
                                tracing::warn!("Accept error: {}", e);
                                continue;
                            }
                        };
                        
                        let handler = http_handler.clone();
                        let mgr = manager.clone();
                        let g = global.clone();
                        let mut socket = socket;
                        
                        tokio::spawn(async move {
                            let mut peek_buf = [0u8; 24];
                            let n = match socket.read(&mut peek_buf).await {
                                Ok(n) => n,
                                Err(_) => return,
                            };
                            if n == 0 { return; }
                            
                            let is_http = peek_buf.starts_with(b"GET ")
                                || peek_buf.starts_with(b"POST ")
                                || peek_buf.starts_with(b"PUT ")
                                || peek_buf.starts_with(b"DELETE ")
                                || peek_buf.starts_with(b"PATCH ")
                                || peek_buf.starts_with(b"HEAD ")
                                || peek_buf.starts_with(b"OPTIONS ");
                            
                            if is_http {
                                let (reader, writer) = socket.into_split();
                                let cursor = Cursor::new(peek_buf[..n].to_vec());
                                let reader = Box::pin(cursor.chain(reader));
                                let reader = BufReader::new(reader);
                                let writer = Box::new(BufWriter::new(writer)) as Box<dyn tokio::io::AsyncWrite + Send + Sync + Unpin>;
                                let boxed_reader: Box<dyn tokio::io::AsyncBufRead + Send + Sync + Unpin> = Box::new(reader);
                                
                                let mut ctx = aex::connection::context::Context::new(
                                    Some(boxed_reader), Some(writer), g, peer_addr,
                                );
                                
                                let handler = handler.clone();
                                let mut ctx = ctx;
                                let result = handler(&mut ctx).await;
                                if result {
                                    let _ = ctx.res().send_response().await;
                                } else {
                                    let _ = ctx.res().send_failure().await;
                                }
                            } else {
                                let pipeline = ConnectionEntry::default_pipeline::<P2PFrame, P2PCommand>(peer_addr, true);
                                let (conn_token, abort_handle, entry_ctx) = ConnectionEntry::start::<_, _>(
                                    mgr.cancel_token.clone(), socket, peer_addr, g, pipeline,
                                );
                                mgr.add(peer_addr, abort_handle, conn_token, true, Some(entry_ctx));
                            }
                        });
                    }
                }
            }
        });

        tracing::info!("CLI started. Type 'help' for commands.");
        let _ = cli.run(reader, ctx).await;
        http_handle.abort();
    }

    /// 核心功能：深度同步活跃连接的元数据到注册表
    pub async fn sync_from_connections(
        &mut self,
        connections: &dashmap::DashMap<SocketAddr, Arc<ConnectionEntry>>,
    ) {
        // 获取当前时间戳用于更新 last_seen
        let now_utc = Utc::now();

        for entry_ref in connections.iter() {
            let addr = *entry_ref.key();
            let entry = entry_ref.value();

            // 1. 识别网络范围
            let current_scope = NetworkScope::from_ip(&addr.ip());
            let registry = if current_scope == NetworkScope::Extranet {
                &mut self.external
            } else {
                &mut self.inner
            };

            // 2. 获取或创建 NodeRecord
            // 使用 take 取出以修改（因为 HashSet 元素具有不可变性限制）
            let mut record = registry
                .nodes
                .take(&NodeRecord::new(addr))
                .unwrap_or_else(|| NodeRecord::new(addr));

            // 3. 基础状态更新
            record.last_seen = now_utc;
            record.is_available = true;
            record.update_status(true);

            // 4. 💡 深度同步：从 ConnectionEntry 的 Option<Node> 中提取信息
            // 注意：这里的 Node 是另一个节点的快照信息，存储在 ConnectionEntry 中
            {
                let node_lock = entry.node.read().await;
                if let Some(peer_info) = &*node_lock {
                    // 同步 ID 和 名称

                    // 如果 peer_info 里有更详细的协议支持，也可以同步
                    record.protocols.extend(peer_info.protocols.iter().cloned());

                    let periods = record.periods.clone();
                    let mut periods1 = record.periods.clone();
                    match periods1.get_mut(periods.clone().len() - 1) {
                        Some(v) => {
                            v.1 = now_utc;
                        }
                        None => {
                            let v = (now_utc, now_utc);
                            record.periods.push(v);
                        }
                    };
                }
            } // 锁在此处释放

            // 5. 放回注册表
            registry.nodes.insert(record);
        }
        let _ = self.save_registries();
    }

    async fn save_registries(&self) -> anyhow::Result<()> {
        self.io_storage
            .save::<HashSet<NodeRecord>>(&self.inner.nodes, STORAGE_INNER_SERVER)
            .await;
        self.io_storage
            .save::<HashSet<NodeRecord>>(&self.external.nodes, STORAGE_EXTERNAL_SERVER)
            .await;
        Ok(())
    }
    pub async fn connect_to(&self, peer_addr: &str) {
        let manager = self.context.manager.clone();
        let global = self.context.clone();

        let endpoint = peer_addr.parse::<SocketAddr>().unwrap();
        let _ = manager
            .connect::<P2PFrame, P2PCommand, _, _>(
                endpoint,
                global,
                move |_ctx| {
                    Box::pin(async move {})
                },
                Some(10),
            )
            .await;

        tracing::info!("Connecting to peer: {}", peer_addr);
    }


}
