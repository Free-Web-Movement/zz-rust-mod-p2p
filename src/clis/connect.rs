use aex::{connection::{global::GlobalContext, node::Node}, tcp::types::Command};
use std::{net::SocketAddr, sync::Arc};

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
            match context
                .clone()
                .manager
                .connect::<P2PFrame, P2PCommand, _, _>(addr, context, move |ctx| async move {
                    println!("Connected to {}!", addr);

                    {
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

                        let aex_node = Node::from_system(addr.port(), id.clone(), 1);

                        let cmd = OnlineCommand {
                            session_id: id,
                            node: aex_node,
                            ephemeral_public_key: key.to_bytes(),
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
                    }
                },
                Arc::new(|cmd: &P2PCommand| { cmd.id() })
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
