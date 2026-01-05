// src/cli.rs
use std::sync::Arc;
use tokio::{
    io::{self, AsyncBufReadExt, BufReader},
    sync::Mutex,
};

use crate::{node::Node, protocols::commands::message::send_text_message};



pub struct Cli {
    node: Arc<Mutex<Node>>,
}

impl Cli {
    pub fn new(node: Arc<Mutex<Node>>) -> Self {
        Self { node }
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        println!("Type 'help' for commands.");

        let stdin = io::stdin();
        let mut reader = BufReader::new(stdin).lines();

        while let Some(line) = reader.next_line().await? {
            let line = line.trim().to_string();
            if line.is_empty() {
                continue;
            }

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

                    let node = self.node.clone();
                    let recv_for_print = receiver.clone();

                    tokio::spawn(async move {
                        let n = node.lock().await;
                        if let Err(e) =
                            send_text_message(receiver, n.context.clone().unwrap(), &msg).await
                        {
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

                        let node = self.node.clone();
                        tokio::spawn(async move {
                            let n = node.lock().await;
                            if let Some(context) = &n.context {
                                let mut servers = context.servers.lock().await;
                                match servers
                                    .connect_to_node(ip.as_str(), port, context)
                                    .await
                                {
                                    Ok(_) => println!("Connected to {}:{}", ip, port),
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
                    let n = self.node.lock().await;
                    println!("Node address: {}", n.address);

                    if let Some(context) = &n.context {
                        let clients = context.clients.lock().await;
                        println!("Connected clients:");
                        for addr in clients.inner.keys() {
                            println!(" - {}", addr);
                        }
                        for addr in clients.external.keys() {
                            println!(" - {}", addr);
                        }

                        let servers = context.servers.lock().await;
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
                    println!(" send <address> <message>   - send text message");
                    println!(" connect <ip> <port>        - connect to a new node");
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
}
