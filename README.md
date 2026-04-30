# zz-rust-mod-p2p

zz rust library for peer to peer connections and interactions.

zz rust 点对点 连接与交互库。

## 目标

1. 支持同一ip，端口下，同时可以响应udp, tcp, http, ws的请求
2. 同时支持以上所有协议下的点对点连接
3. 支持webrtc下的点对点连接方式
4. 开发透明的点对点协议，只需要按点对点协议编写代码，就可以提供四种协议下的点对点服务。
5. 提供点对点的应用平台基础。包括(消息，音频，视频)。

## 技术设定

1. 将同一个节点的所有网络连接分为服务端节点与客户端节点，统一管理
2. 将不同的服务器端单独列出来进行管理，也就是一个节点要同时管理 udp, tcp, http, web socket等至少四种服务器，让这个服务器，即可以服务于纯tcp的连接，也可以是http的连接，以及web socket的连接
3. session共享，对于http连接来说，分享用户登录信息是一件比较麻烦的事情，但是在free web movement的p2p项目里，他通过Address里的公钥与地址系统可以实现session的共享
4. 为不同的协议设定应用层面的统一接口。

## 功能特性

### 核心功能

- **多协议支持**: 同时支持 TCP、UDP、HTTP、WebSocket
- **P2P 加密通信**: 基于 ChaCha20Poly1305 + X25519 + Ed25519 的端到端加密
- **心跳检测**: 自动心跳保活，支持超时检测和延迟监控
- **连接管理**: 入站/出站连接统一管理，支持内外网分离
- **节点注册表**: 持久化存储节点信息，支持失效检测

### 协议层

- **P2PFrame**: 安全帧结构，含签名验证
- **P2PCommand**: 命令系统，支持 Entity/Action 模式
  - Node: OnLine, OffLine, OnLineAck, Update
  - Message: SendText, SendBinary

### CLI 命令

- `connect <ip> <port>` - 连接到远程节点
- `send <message>` - 发送消息
- `status` - 查看连接状态
- `help` - 查看帮助

## 架构图景

```
┌────────────────┐
│    CLI/UI      │   ← 人类操作
└────┬───────────┘
     │
┌────▼──────────────────────┐
│           Node             │  ← 节点生命周期 & 协调者
└────┬──────────────────────┘
     │
┌────▼──────────┐  ┌────────▼────────┐
│   TCPHandler  │  │   UDPHandler     │  ← 网络 IO
└────┬──────────┘  └────────┬────────┘
     │                         │
┌────▼─────────────────────────▼─────┐
│              Context                │  ← 全局共享状态
└────┬─────────────────────────┬─────┘
     │                         │
┌────▼──────────────┐   ┌──────▼───────────┐
│ ConnectedClients  │   │ ConnectedServers │
└────┬──────────────┘   └──────┬───────────┘
     │                         │
┌────▼───────────────────────────────────┐
│        Protocol Layer (Frame / Command) │
└────┬───────────────────────────────────┘
     │
┌────▼─────────────────────────┐
│ MessageCommand/EventCommand  │ ← 业务层（文本消息）
└──────────────────────────────┘

```

## 使用示例

```rust
use zz_p2p::{cli::Opt, node::Node};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let stdin = tokio::io::stdin();
    let reader = tokio::io::BufReader::new(stdin);
    let mut node = Node::init(Opt::parse()).await;
    node.start(reader).await;
    Ok(())
}
```

## 依赖

- `tokio` - 异步运行时
- `aex` - 网络框架 (TCP/UDP/HTTP/WebSocket)
- `zz-account` - 账户与地址系统
- `bitcoin` - 比特币相关类型
- `chacha20poly1305` - 流加密
- `x25519-dalek` - 密钥交换
- `ed25519-dalek` - 数字签名

## 版本

当前版本: 0.1.6
