use aex::connection::{global::GlobalContext, node::Node};
use std::{net::SocketAddr, sync::Arc};

use crate::protocols::{
    command::{Action, Entity, P2PCommand},
    commands::online::{get_all_ips, OnlineCommand},
    frame::P2PFrame,
};
use crate::protocols::commands::ack::{SeedRecord, SeedsCommand};

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

                            let aex_node = Node::from_system(peer.port(), id.clone(), 1);
                            let announced_ips = get_all_ips();

                            // Build seeds to send: current connected peers + self
                            let seeds_to_send = {
                                let guard = ctx_for_seeds.lock().await;
                                let connected: Vec<String> = guard.global.manager.get_all_entries()
                                    .iter()
                                    .map(|a| a.to_string())
                                    .collect();
                                let self_addr = guard.global.addr.to_string();
                                drop(guard);

                                let mut all_seeds: Vec<SeedRecord> = connected
                                    .iter()
                                    .filter(|s| *s != &self_addr)
                                    .map(|addr| SeedRecord::new(addr.clone()))
                                    .collect();
                                all_seeds.push(SeedRecord::new(self_addr.clone()));
                                SeedsCommand::new(all_seeds)
                            };

                            let cmd = OnlineCommand {
                                session_id: id,
                                node: aex_node,
                                ephemeral_public_key: key.to_bytes(),
                                announced_ips,
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
