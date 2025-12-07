use serde_json::Value;
use zz_account::address::FreeWebMovementAddress as Address;


trait NatOperations {
    fn punch_hole(&self, target_ip: &str, target_port: u16) -> anyhow::Result<()>;
    fn maintain_nat(&self) -> anyhow::Result<()>;
    fn plug(&self, one: &(String, u16), another: &(String, u16)) -> bool;
    fn unplug(&self, one: &(String, u16), another: &(String, u16)) -> bool;
}
trait NetService<T> {
    fn start(&self);
    fn stop(&self);
    fn send(&self, stream: &mut T, data: &[u8]);
    fn receive(&self, stream: &mut T) -> Vec<u8>;
    fn process(&self, context: &mut Context<T>);
    fn status(&self) -> String;
}

#[allow(non_camel_case_types)]
enum NatProtocol {
    STUN,
    TURN,
    ICE,
    HOLE_PUNCH,
}

// Net Service Type
#[allow(non_camel_case_types)]
enum TransportProtocol {
    UDP,
    TCP,
    HTTP,
    HTTP2,
    QUIC,
    HTTP3,
    WEB_SOCKET,
    // SMTP,
    // FTP,
    // ICMP,
    // SCTP,
    // SSH,
    // DNS,
}

struct NatInfo {
    pub stun_port: u16,
    pub turn_port: u16,
}


struct Node {
    pub ip: String,
    pub port: u16,
    pub address: Address,
    pub stun_port: u16,
    pub turn_port: u16,
    pub last_seen: u64,
}

struct NatPair<S, T> {
    pub protocol: NatProtocol,
    server: S, // One Server/Listener Multiple Clients
    clients: Vec<T>,
    plugged_pairs: Vec<(T, T)>,
}

struct Context<'a, T> {
    node: Node,
    stream: &'a mut T, // TCP/UDP Stream
    global: Value,
    local: Value,
}
