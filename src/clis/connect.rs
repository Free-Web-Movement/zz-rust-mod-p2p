use aex::connection::{global::GlobalContext, node::Node};
use std::{net::SocketAddr, sync::Arc};

use crate::protocols::{
    command::{Action, Entity, P2PCommand},
    commands::online::{get_all_ips, OnlineCommand},
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
            match manager
                .connect::<P2PFrame, P2PCommand, _, _>(
                    addr,
                    global.clone(),
                    move |ctx| {
                        let peer = addr;
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

                            let cmd = OnlineCommand {
                                session_id: id,
                                node: aex_node,
                                ephemeral_public_key: key.to_bytes(),
                                announced_ips,
                                seeds: None,
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
