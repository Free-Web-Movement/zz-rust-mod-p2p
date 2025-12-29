use clap::Parser;
use std::{path::PathBuf, sync::Arc};
use tokio::{
    io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader},
    sync::Mutex,
};
use zz_account::address::FreeWebMovementAddress as Address;
use zz_p2p::{node::Node, nodes::storage::Storeage};

#[derive(Parser, Debug)]
#[command(name = "zzp2p")]
struct Opt {
    /// IP 地址，例如 0.0.0.0
    #[arg(long, default_value = "zz-p2p-node")]
    name: String,

    /// IP 地址，例如 0.0.0.0
    #[arg(long, default_value = "0.0.0.0")]
    ip: String,

    /// TCP 端口
    #[arg(long, default_value_t = 9000)]
    port: u16,

    /// 数据目录，用于存储节点持久化信息
    #[arg(long)]
    data_dir: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opt = Opt::parse();

    let storage = Storeage::new(opt.data_dir.as_deref(), None, None, None);

    // 获取或生成节点 address
    let address = if let Some(addr) = storage.read_address()? {
        println!("Using existing address: {}", &addr);
        addr
    } else {
        let addr = Address::random();
        println!("Generated new address: {}", &addr);
        storage.save_address(&addr)?;
        addr
    };

    // 初始化 Node
    let node = Arc::new(Mutex::new(Node::new(
        opt.name.clone(),
        address,
        opt.ip.clone(),
        opt.port,
        Some(storage),
    )));

    // 启动节点
    {
        let node_clone = node.clone();
        tokio::spawn(async move {
            let mut n = node_clone.lock().await;
            n.start().await;
        });
    }

    println!("Node started at {}:{}", opt.ip, opt.port);
    println!("Type 'help' for commands.");

    // REPL 命令行
    let stdin = io::stdin();
    let mut reader = BufReader::new(stdin).lines();

    while let Some(line) = reader.next_line().await? {
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }

        // produce owned Strings so substrings can be moved into async tasks safely
        let mut parts = line.splitn(3, ' ').map(str::to_string);
        let command = parts.next().unwrap();
        match command.as_str() {
            "send" => {
                let receiver = match parts.next() {
                    Some(a) => a,
                    None => {
                        println!("Usage: send <address> <message>");
                        continue;
                    }
                };
                let msg = match parts.next() {
                    Some(m) => m,
                    None => {
                        println!("Usage: send <address> <message>");
                        continue;
                    }
                };

                let node_clone = node.clone();
                let recv_for_print = receiver.clone();
                tokio::spawn(async move {
                    let n = node_clone.lock().await;
                    if let Err(e) = n.send_text_message(receiver, &msg).await {
                        println!("Failed to send message: {:?}", e);
                    } else {
                        println!("Message sent to {}", recv_for_print);
                    }
                });
            }

            "connect" => {
                let ip = parts.next();
                let port_str = parts.next();
                if let (Some(ip), Some(port_str)) = (ip, port_str) {
                    let port: u16 = match port_str.parse() {
                        Ok(p) => p,
                        Err(_) => {
                            println!("Invalid port: {}", port_str);
                            continue;
                        }
                    };

                    let node_clone = node.clone();
                    tokio::spawn(async move {
                        let mut n = node_clone.lock().await;
                        if let Some(servers) = &mut n.servers {
                            // 调用你的连接函数
                            match servers.connect_to_node(ip.as_str(), port).await {
                                Ok(_) => {
                                    println!("Connected to {}:{}", ip, port);
                                    n.notify_online();
                                }
                                Err(e) => println!("Failed to connect: {:?}", e),
                            }
                        } else {
                            println!("Servers not initialized");
                        }
                    });
                } else {
                    println!("Usage: connect <ip> <port>");
                }
            }

            "status" => {
                let n = node.lock().await;
                println!("Node address: {}", n.address);

                if let Some(context) = &n.context {
                    let clients = context.clients.lock().await;
                    println!("Connected clients (inner + external):");
                    for addr in clients.inner.keys() {
                        println!(" - {}", addr);
                    }
                    for addr in clients.external.keys() {
                        println!(" - {}", addr);
                    }
                }

                if let Some(servers) = &n.servers {
                    println!("Connected servers:");
                    if let Some(connected) = &servers.connected_servers {
                        for s in connected.inner.iter().chain(connected.external.iter()) {
                            println!(" - {}", s.record.endpoint);
                        }
                    }
                }
            }

            "help" => {
                println!("Commands:");
                println!(" send <address> <message>  - send text message");
                println!(" connect <ip> <port>       - connect to a new node");
                println!(" status                     - show connected clients and servers");
                println!(" exit                       - exit the program");
            }

            "exit" => {
                println!("Exiting...");
                break;
            }

            _ => {
                println!(
                    "Unknown command: '{}', type 'help' for available commands",
                    command
                );
            }
        }
    }

    Ok(())
}
