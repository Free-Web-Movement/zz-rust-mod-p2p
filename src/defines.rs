use serde_json::Value;

struct context_t<T> {
    ip: String,
    port: u16,
    stream: &mut <T>,   // TCP/UDP Stream
    global: Value,
    local: Value
}