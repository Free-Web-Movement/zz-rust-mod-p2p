use serde_json::Value;
use zz_account::FreeWebMovementAdddress as Address;

enum NetType {
    UDP,
    TCP,
    HTTP,
    Http2,
    WebSocket
}

struct NetClient {
    net_type: NetType
}

struct NetServer <T> {
    ip: String,
    port: u16,
    address: Address,
    listeners: &mut GenericArray<T>,
}

struct Context<T> {
    stream: &mut T,   // TCP/UDP Stream
    global: Value,
    local: Value
}