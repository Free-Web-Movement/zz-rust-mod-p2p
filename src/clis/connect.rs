use aex::connection::{global::GlobalContext, scope::NetworkScope};
use std::{net::SocketAddr, sync::Arc};

use crate::node::Node as P2pNode;
use crate::protocols::commands::ack::{SeedRecord, SeedsCommand};
use crate::protocols::{
    command::{Action, Entity, P2PCommand},
    commands::online::OnlineCommand,
    frame::P2PFrame,
};

pub async fn handle(args: Vec<String>, context: Arc<GlobalContext>) {
    if args.len() < 2 {
        println!("Usage: connect <ip> <port>");
        return;
    }
    let addr_str = format!("{}:{}", args[0], args[1]);
    match addr_str.parse::<SocketAddr>() {
        Ok(addr) => {
            let manager = context.manager.clone();
            let global = context.clone();

            // Register peer in NodeRegistry
            if let Some(node) = global.get::<Arc<P2pNode>>().await {
                let self_node_id = global.local_node.read().await.id.clone();
                let self_address = String::from_utf8(self_node_id).unwrap_or_default();
                let scope = NetworkScope::from_ip(&addr.ip());
                node.registry.register(self_address, addr, scope);
            }

            match manager
                .connect::<P2PFrame, P2PCommand, _, _>(
                    addr,
                    global.clone(),
                    move |ctx| {
                        let peer = addr;
                        let ctx_for_seeds = ctx.clone();
                        Box::pin(async move {
                            println!("Connected to {}!", peer);

                            let psk = {
                                let guard = ctx.lock().await;
                                let g = guard.global.clone();
                                g.paired_session_keys.clone().unwrap()
                            };

                            let (id, key) = {
                                let cloned = psk.clone();
                                let guard = cloned.lock().await;
                                guard.create(false).await
                            };

                            let aex_node = {
                                let guard = ctx.lock().await;
                                guard.global.local_node.read().await.clone()
                            };
                            let (intranet_ips, wan_ips) = {
                                let mut inner = Vec::new();
                                let mut outer = Vec::new();
                                for (scope, ip) in &aex_node.ips {
                                    match scope {
                                        aex::connection::scope::NetworkScope::Intranet => {
                                            inner.push(ip.to_string())
                                        }
                                        aex::connection::scope::NetworkScope::Extranet => {
                                            outer.push(ip.to_string())
                                        }
                                    }
                                }
                                (inner, outer)
                            };

                            // Build seeds from NodeRegistry
                            let seeds_to_send = {
                                let guard = ctx_for_seeds.lock().await;
                                let seeds =
                                    if let Some(node) = guard.global.get::<Arc<P2pNode>>().await {
                                        let all_seeds: Vec<SeedRecord> = node
                                            .registry
                                            .get_all_seeds()
                                            .into_iter()
                                            .map(|(s, na)| SeedRecord::new(s.to_string(), na))
                                            .collect();
                                        SeedsCommand::new(all_seeds)
                                    } else {
                                        SeedsCommand::new(vec![])
                                    };
                                drop(guard);
                                seeds
                            };

                            let cmd = OnlineCommand {
                                session_id: id,
                                node: aex_node,
                                ephemeral_public_key: key.to_bytes(),
                                intranet_ips,
                                wan_ips,
                                seeds: Some(seeds_to_send),
                            };
                            P2PFrame::send::<OnlineCommand>(
                                ctx.clone(),
                                &Some(cmd),
                                Entity::Node,
                                Action::OnLine,
                                false,
                            )
                            .await
                            .expect("Online Command Sending Failed!");
                            println!("message send!");
                        })
                    },
                    Some(10),
                )
                .await
            {
                Ok(_) => println!("Connection attempt started..."),
                Err(e) => println!("Failed to connect: {:?}", e),
            }
        }
        Err(_) => println!("Invalid address: {}", addr_str),
    }
}
