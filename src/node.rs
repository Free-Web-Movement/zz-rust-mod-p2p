use aex::{
    connection::{
        entry::ConnectionEntry, global::GlobalContext, heartbeat::HeartbeatConfig,
        node::Node as AexNode, scope::NetworkScope,
    },
    crypto::session_key_manager::PairedSessionKey,
    server::{HTTPServer, Server},
    storage::Storage,
    tcp::router::Router as TcpRouter,
    unified::UnifiedServer,
};
use chrono::Utc;
use futures::future::FutureExt;
use std::{collections::HashSet, net::SocketAddr, sync::Arc};
use tokio::sync::{Mutex, RwLock};
use zz_account::address::FreeWebMovementAddress;

use crate::{
    cli::{Cli, Opt},
    io_storage::{
        IOStorage, STORAGE_ADDRESS, STORAGE_EXTERNAL_SERVER, STORAGE_INNER_SERVER, io_storage_init,
    },
    protocols::{command::{P2PCommand, Action, Entity}, frame::P2PFrame, registry::register},
    protocols::commands::node_registry::NodeRegistry,
    record::{self, NodeRecord},
};

pub type WebHandler = Arc<
    dyn Fn(&mut aex::connection::context::Context) -> futures::future::BoxFuture<'_, bool>
        + Send
        + Sync,
>;

// 用于保存节点的所有信息
// 用于当前程序的基本信息共享

#[derive(Clone)]
pub struct Node {
    pub id: FreeWebMovementAddress,
    pub inner: record::NodeRegistry,
    pub external: record::NodeRegistry,
    pub registry: NodeRegistry,
    pub peer_addrs: Arc<RwLock<Vec<SocketAddr>>>,
    pub io_storage: IOStorage,
    pub name: String,
    pub addr: SocketAddr,
    pub context: Arc<GlobalContext>,
    pub server: Server,
    pub cli: Arc<Cli>,
}

impl Node {
    pub async fn new(
        name: String,
        io_storage: IOStorage,
        addr: SocketAddr,
        context: Arc<GlobalContext>,
        server: Server,
        cli: Arc<Cli>,
        registry: NodeRegistry,
        peer_addrs: Arc<RwLock<Vec<SocketAddr>>>,
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
        let inner = record::NodeRegistry::new(inner_nodes);
        let external = record::NodeRegistry::new(external_nodes);
        Self {
            name,
            id,
            inner,
            external,
            registry,
            peer_addrs,
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
        let self_registry = self.registry.clone();

        let nodes: Vec<record::NodeRecord> = self.inner.nodes.iter().cloned().collect();

        for record in nodes {
            let endpoint = record.endpoint;
            let g = global.clone();
            let registry = self_registry.clone();

            let _ = manager
                .connect::<P2PFrame, P2PCommand, _, _>(
                    endpoint,
                    g,
                    move |ctx| {
                        let peer = endpoint;
                        let self_registry = registry.clone();
                        Box::pin(async move {
                            tracing::info!("✅ Connected to peer: {}", peer);

                            let psk = {
                                let guard = ctx.lock().await;
                                let g = guard.global.clone();
                                g.paired_session_keys.clone().unwrap()
                            };

                            let (id, key) = {
                                let guard = psk.lock().await;
                                guard.create(false).await
                            };

                            // Get local_node.id (FreeWebMovementAddress bytes)
                            let self_node_id = {
                                let guard = ctx.lock().await;
                                guard.global.local_node.read().await.id.clone()
                            };

                            let aex_node = AexNode::from_system(peer.port(), self_node_id.clone(), 1);

                            // Generate seeds from NodeRegistry
                            let seeds_to_send = {
                                let all_seeds: Vec<crate::protocols::commands::ack::SeedRecord> = self_registry
                                    .get_all_seeds()
                                    .into_iter()
                                    .map(|(s, na)| crate::protocols::commands::ack::SeedRecord::new(s.to_string(), na))
                                    .collect();
                                crate::protocols::commands::ack::SeedsCommand::new(all_seeds)
                            };

                            let (intranet_ips, wan_ips) = crate::protocols::commands::online::get_all_ips();
                            let cmd = crate::protocols::commands::online::OnlineCommand {
                                session_id: id,
                                node: aex_node,
                                ephemeral_public_key: key.to_bytes(),
                                intranet_ips,
                                wan_ips,
                                seeds: Some(seeds_to_send),
                            };
                            if let Err(e) = P2PFrame::send::<crate::protocols::commands::online::OnlineCommand>(
                                ctx.clone(),
                                &Some(cmd),
                                Entity::Node,
                                Action::OnLine,
                                false,
                            ).await {
                                tracing::error!("Failed to send OnlineCommand: {:?}", e);
                            }

                            // Start reader loop to process responses (OnlineAck, seeds, etc.)
                            let g = {
                                let guard = ctx.lock().await;
                                guard.global.clone()
                            };
                            if let Some(router) = aex::connection::context::get_tcp_router::<P2PFrame, P2PCommand>(&g.routers) {
                                let _ = router.handle(ctx).await;
                            }
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
        let io_storage = io_storage_init(&opt, storage.clone());

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

        // Set local_node.id = FreeWebMovementAddress bytes
        {
            let mut local_node = global.local_node.write().await;
            local_node.id = address.to_string().into_bytes();
        }

        global.set(address.clone()).await;

        let address_1 = global.get::<FreeWebMovementAddress>().await.unwrap();
        assert_eq!(address.to_string(), address_1.to_string());
        global.set(storage.clone()).await;
        global.set(io_storage.clone()).await;
        // 初始化消息去重集合
        let seen: crate::protocols::commands::message::SeenMessages =
            Arc::new(std::sync::Mutex::new(std::collections::HashSet::new()));
        global.set(seen).await;
        // 初始化待确认回执表
        global.set(crate::protocols::commands::message::PendingAcks::default()).await;
        let cli = Cli::new();

        let server = HTTPServer::new(addr, Some(global.clone()));

        let router = TcpRouter::<P2PFrame, P2PCommand>::new();

        let router = register(router);
        let server = server.tcp(router);

        // Create NodeRegistry and register self seeds
        let node_registry = NodeRegistry::new();
        let all_ips = aex::connection::node::Node::system_ips();
        let self_address = address.to_string();

        let is_loopback = addr.ip().is_loopback();
        if !is_loopback {
            let scope = NetworkScope::from_ip(&addr.ip());
            node_registry.register(self_address.clone(), addr, scope);
            tracing::info!("🌱 Registered self seed: {} (node: {})", addr, self_address);
        }

        for (_, ip) in &all_ips {
            if ip == &addr.ip() {
                continue;
            }
            if ip.is_loopback() {
                continue;
            }
            let seed_addr = SocketAddr::new(*ip, addr.port());
            let scope = NetworkScope::from_ip(ip);
            node_registry.register(self_address.clone(), seed_addr, scope);
            tracing::info!("🌱 Registered self seed: {} (node: {})", seed_addr, self_address);
        }

        // Create peer_addrs from CLI seeds
        let seed_addrs: Vec<SocketAddr> = if let Some(ref seeds_str) = opt.seeds {
            seeds_str
                .split(',')
                .filter_map(|s| s.trim().parse::<SocketAddr>().ok())
                .collect()
        } else {
            vec![]
        };

        let mut node = Node::new(
            opt.name,
            io_storage,
            addr,
            global.clone(),
            server,
            Arc::new(cli),
            node_registry,
            Arc::new(RwLock::new(seed_addrs.clone())),
        )
        .await;

        // Store Arc<Node> in GlobalContext
        let node_arc = Arc::new(node.clone());
        global.set(node_arc).await;

        // Save CLI seeds to persistent registries
        if opt.seeds.is_some() {
            for saddr in &seed_addrs {
                node.inner.upsert(*saddr, true);
                node.external.upsert(*saddr, true);
                tracing::info!("Adding seed: {}", saddr);
            }
            let _ = node.save_registries().await;
        }

        if opt.test {
            tracing::info!("Test mode: node {} ready (displayed via manager)", opt.port);
        }

        node
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

    pub async fn start_with_web<R>(self, _reader: R, web_handler: WebHandler)
    where
        R: tokio::io::AsyncBufRead + Unpin + Send + 'static,
    {
        let addr = self.addr;
        let globals = self.context.clone();
        let handler = Arc::new(web_handler);

        let tcp_router = Arc::new(register(TcpRouter::<P2PFrame, P2PCommand>::new()));

        let unified = UnifiedServer::new(addr, globals)
            .http_router({
                let mut router =
                    aex::http::router::Router::new(aex::http::router::NodeType::Static("root".into()));
                let h = handler.clone();
                let executor: std::sync::Arc<
                    dyn for<'a> std::ops::Fn(
                            &'a mut aex::connection::context::Context,
                        )
                            -> futures::future::BoxFuture<'a, bool>
                        + Send
                        + Sync,
                > = std::sync::Arc::new(move |ctx: &mut aex::connection::context::Context| {
                    let hh = h.clone();
                    async move { hh(ctx).await }.boxed()
                });
                router.get("/", executor).register();

                let h2 = handler.clone();
                let api_executor: std::sync::Arc<
                    dyn for<'a> std::ops::Fn(
                            &'a mut aex::connection::context::Context,
                        )
                            -> futures::future::BoxFuture<'a, bool>
                        + Send
                        + Sync,
                > = std::sync::Arc::new(move |ctx: &mut aex::connection::context::Context| {
                    let hh2 = h2.clone();
                    async move { hh2(ctx).await }.boxed()
                });
                router.get("/api/data", api_executor).register();

                // Catch-all so API endpoints (POST /api/send_chat, etc.) reach the web handler
                let h3 = handler.clone();
                let catch_all_executor: std::sync::Arc<
                    dyn for<'a> std::ops::Fn(
                            &'a mut aex::connection::context::Context,
                        )
                            -> futures::future::BoxFuture<'a, bool>
                        + Send
                        + Sync,
                > = std::sync::Arc::new(move |ctx: &mut aex::connection::context::Context| {
                    let hh3 = h3.clone();
                    async move { hh3(ctx).await }.boxed()
                });
                router.all("/*", catch_all_executor).register();

                // WebSocket route for real-time push notifications
                let ws_middleware = aex::http::middlewares::websocket::WebSocket::to_middleware(
                    aex::http::middlewares::websocket::WebSocket::new()
                );
                let ws_executor: std::sync::Arc<
                    dyn for<'a> std::ops::Fn(
                            &'a mut aex::connection::context::Context,
                        )
                            -> futures::future::BoxFuture<'a, bool>
                        + Send
                        + Sync,
                > = std::sync::Arc::new(move |_ctx: &mut aex::connection::context::Context| {
                    async move { true }.boxed()
                });
                router.get("/ws", ws_executor).middleware(Arc::from(ws_middleware)).register();
                router
            })
            .tcp_handler(Arc::new(move |ctx| {
                let router = tcp_router.clone();
                let peer_addr = ctx.addr;
                let manager = ctx.global.manager.clone();
                let ctx_arc = Arc::new(Mutex::new(ctx));
                let ctx_for_add = ctx_arc.clone();

                let child_token = manager.cancel_token.child_token();
                let task_token = child_token.clone();

                let handle = tokio::spawn(async move {
                    tokio::select! {
                        _ = task_token.cancelled() => {}
                        _ = router.handle(ctx_arc) => {}
                    }
                });

                let abort_handle = handle.abort_handle();
                manager.add(peer_addr, abort_handle, child_token, true, Some(ctx_for_add));

                handle
            }));

        tracing::info!("Server running. Press Ctrl+C to stop.");
        let _ = unified.start().await;
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
    pub async fn connect_to(&mut self, peer_addr: &str) -> Result<(), String> {
        let endpoint = peer_addr.parse::<SocketAddr>().map_err(|e| e.to_string())?;
        
        // Add to both inner and external seeds
        self.inner.upsert(endpoint, true);
        self.external.upsert(endpoint, true);
        
        let manager = self.context.manager.clone();
        let global = self.context.clone();

        manager
            .connect::<P2PFrame, P2PCommand, _, _>(
                endpoint,
                global,
                move |_ctx| Box::pin(async move {}),
                Some(10),
            )
            .await
            .map_err(|e| e.to_string())?;

        tracing::info!("Connecting to peer: {}", peer_addr);
        
        // Save to storage
        let _ = self.save_registries().await;
        
        Ok(())
    }
}

pub fn is_public_ip(ip: &std::net::IpAddr) -> bool {
    !ip.is_loopback() && !ip.is_unspecified()
}

pub fn is_public_addr(addr: &SocketAddr) -> bool {
    is_public_ip(&addr.ip())
}

pub fn filter_entries(addrs: Vec<SocketAddr>) -> Vec<SocketAddr> {
    addrs.into_iter().filter(is_public_addr).collect()
}
