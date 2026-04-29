use std::net::SocketAddr;
use std::sync::Arc;

use aex::connection::context::Context;
use aex::connection::manager::ConnectionManager;
use aex::connection::node::Node;
use aex::tcp::types::Codec;
use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;
use zz_account::address::FreeWebMovementAddress;

use crate::protocols::command::P2PCommand;
use crate::protocols::command::{Action, Entity};
use crate::protocols::commands::ack::{OnlineAckCommand, SeedRecord, SeedsCommand};
use crate::protocols::frame::P2PFrame;

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct OnlineCommand {
    pub session_id: Vec<u8>,
    pub node: Node,
    pub ephemeral_public_key: [u8; 32],
    pub announced_ips: Vec<String>,
    pub seeds: Option<SeedsCommand>,
}

impl Codec for OnlineCommand {}

pub async fn online_handler(
    ctx: Arc<Mutex<Context>>,
    frame: P2PFrame,
    cmd: P2PCommand,
) {
    println!("inside online handler!");
    let online: OnlineCommand = match Codec::decode(&cmd.data) {
        Ok(cmd) => cmd,
        Err(e) => {
            eprintln!("❌ decode OnlineCommand failed: {e}");
            return ();
        }
    };
    println!(
        "✅ Node Online: addr={}, nonce={}",
        frame.body.address, frame.body.nonce
    );

    println!("received session_id: {:?}", online.session_id);
    println!("announced IPs: {:?}", online.announced_ips);
    println!("received seeds: {:?}", online.seeds.as_ref().map(|s| s.seeds.len()));

    let psk = {
        let ctx = ctx.lock().await;
        ctx.global.paired_session_keys.clone().unwrap()
    };

    let ephemeral_public = {
        let guard = psk.lock().await;
        guard
            .establish_begins(
                frame.body.address.as_bytes().to_vec(),
                &online.ephemeral_public_key.to_vec(),
            )
            .await
            .unwrap()
            .unwrap()
    };

    let address: FreeWebMovementAddress = {
        let ctx = ctx.lock().await;
        ctx.global.get().await.expect("Expect Address be set!")
    };

    let local_node = {
        let ctx = ctx.lock().await;
        ctx.global.local_node.clone()
    };

    let node = {
        let guard = local_node.write().await;
        guard.clone()
    };

    let announced_ips = get_all_ips();
    println!("Announcing IPs: {:?}", announced_ips);

    // Process seeds consensus: merge peer's seeds first, then compute our combined seeds
    let seeds_to_send = {
        let guard = ctx.lock().await;
        let manager = guard.global.manager.clone();
        let local_addr = guard.global.addr;
        let entries = manager.get_all_entries();
        println!("📊 Current manager entries: {} nodes", entries.len());

        // Build combined seeds list
        let mut all_seeds: Vec<String> = entries
            .iter()
            .map(|addr| addr.to_string())
            .collect();

        // Merge peer's seeds if received
        if let Some(ref peer_seeds) = online.seeds {
            if peer_seeds.verify() {
                println!("🔄 Merging peer's seeds: {} nodes, hash={:?}", peer_seeds.seeds.len(), peer_seeds.hash);
                for seed in &peer_seeds.seeds {
                    if !all_seeds.contains(&seed.address) {
                        all_seeds.push(seed.address.clone());
                        println!("  + Added seed from peer: {}", seed.address);
                    }
                }
            } else {
                eprintln!("❌ Invalid seeds hash from peer!");
            }
        }

        // Add self as seed
        let self_addr = local_addr.to_string();
        if !all_seeds.contains(&self_addr) {
            all_seeds.push(self_addr.clone());
            println!("  + Added self as seed: {}", self_addr);
        }

        // Build seed records
        let seed_records: Vec<SeedRecord> = all_seeds
            .iter()
            .map(|addr| SeedRecord::new(addr.clone()))
            .collect();

        let cmd = SeedsCommand::new(seed_records);
        println!("📊 Consensus seeds: {} nodes, hash={:?}", cmd.seeds.len(), cmd.hash);

        Some(cmd)
    };

    let ack = OnlineAckCommand {
        session_id: online.session_id,
        address: address.to_string(),
        node,
        ephemeral_public_key: ephemeral_public.to_bytes(),
        announced_ips,
        seeds: seeds_to_send,
    };

    println!("send ack session_id : {:?}", ack.session_id);
    println!("send ack: {:?}", Codec::encode(&ack));

    P2PFrame::send::<OnlineAckCommand>(
        ctx.clone(),
        &Some(ack),
        Entity::Node,
        Action::OnLineAck,
        false,
    )
    .await
    .expect("Error send online ack!");
    println!("end of current online!");

    // Store the announced IPs from peer as external seeds
    for ip in online.announced_ips {
        if ip != "0.0.0.0" && !ip.starts_with("127.") {
            println!("Received external IP from peer: {}", ip);
        }
    }

    println!("end of online!");
}

pub fn get_all_ips() -> Vec<String> {
    use std::process::Command;
    let output = Command::new("ip")
        .args(&["route", "get", "1.1.1.1"])
        .output();
    
    let mut ips = vec![];
    
    if let Ok(output) = output {
        if let Ok(ip_str) = String::from_utf8(output.stdout) {
            for line in ip_str.lines() {
                if let Some(src) = line.strip_prefix("src ") {
                    let ip = src.trim().to_string();
                    if !ip.starts_with("127.") && ip != "0.0.0.0" {
                        ips.push(ip);
                    }
                }
            }
        }
    }
    
    // Fallback
    if ips.is_empty() {
        if let Ok(output) = Command::new("ip").arg("addr").output() {
            if let Ok(output_str) = String::from_utf8(output.stdout) {
                for line in output_str.lines() {
                    if line.contains("inet ") && !line.contains("127.") {
                        if let Some(ip_start) = line.find("inet ") {
                            let rest = &line[ip_start + 5..];
                            if let Some(ip) = rest.split_whitespace().next() {
                                let ip = ip.split('/').next().unwrap_or("").to_string();
                                if !ip.starts_with("127.") && !ip.starts_with("169.254") && ip != "0.0.0.0" {
                                    ips.push(ip);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    ips
}
