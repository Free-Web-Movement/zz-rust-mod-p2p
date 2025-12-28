use std::{path::PathBuf, sync::Arc};
use clap::Parser;
use tokio::{io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader}, sync::Mutex};
use zz_p2p::{node::Node, nodes::storage::Storeage};
use zz_account::address::FreeWebMovementAddress as Address;


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
        addr
    } else {
        Address::random()
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

        let mut parts = line.splitn(3, ' ');
        let command = parts.next().unwrap();
        match command {
            "send" => {
                let receiver = match parts.next() {
                    Some(a) => a.to_string(),
                    None => {
                        println!("Usage: send <address> <message>");
                        continue;
                    }
                };
                let msg = match parts.next() {
                    Some(m) => m.to_string(),
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
                println!(" status                     - show connected clients and servers");
                println!(" exit                       - exit the program");
            }

            "exit" => {
                println!("Exiting...");
                break;
            }

            _ => {
                println!("Unknown command: '{}', type 'help' for available commands", command);
            }
        }
    }

    Ok(())
}
