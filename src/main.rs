use clap::Parser;
use tokio_util::sync::CancellationToken;

/// 简单 TCP + UDP 服务器参数
#[derive(Parser, Debug)]
#[command(name = "free_wm_p2p")]
struct Opt {
    /// IP 地址，例如 0.0.0.0
    #[arg(long, default_value = "0.0.0.0")]
    ip: String,

    /// TCP 端口
    #[arg(long, default_value_t = 9000)]
    tcp_port: u16,

    /// UDP 端口
    #[arg(long, default_value_t = 9000)]
    udp_port: u16,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opt = Opt::parse();

    let address = zz_account::address::FreeWebMovementAddress::random();
    let mut node =
        zz_p2p::node::Node::new("node1".to_owned(), address, opt.ip.clone(), opt.tcp_port);
    node.start().await;

    // 阻塞主线程
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
    }
}
