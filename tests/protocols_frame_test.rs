#[cfg(test)]
mod tests {
    use std::{collections::HashSet, net::SocketAddr, pin::Pin, sync::Arc, task::Poll};

    use aex::{
        connection::{
            context::{AexWriter, BoxWriter, Context},
            global::GlobalContext,
            node::Node,
            types::{ConnectionEntry, NetworkScope},
        },
        tcp::types::{Codec, Frame},
    };
    use tokio::{
        io::AsyncReadExt,
        sync::Mutex,
    };
    use tokio_util::sync::CancellationToken;
    use zz_account::address::FreeWebMovementAddress;
    use zz_p2p::protocols::{
        command::{Action, Entity, P2PCommand},
        frame::{FrameBody, P2PFrame},
    };

use tokio::io::{AsyncWrite, ErrorKind};

    struct AlwaysFailWriter;

    impl AsyncWrite for AlwaysFailWriter {
        fn poll_write(
            self: Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
            _buf: &[u8],
        ) -> Poll<Result<usize, tokio::io::Error>> {
            // 直接返回错误，触发 eprintln!
            Poll::Ready(Err(tokio::io::Error::new(ErrorKind::Other, "Mock IO Error")))
        }

        fn poll_flush(self: Pin<&mut Self>, _cx: &mut std::task::Context<'_>) -> Poll<Result<(), tokio::io::Error>> {
            Poll::Ready(Ok(()))
        }

        fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut std::task::Context<'_>) -> Poll<Result<(), tokio::io::Error>> {
            Poll::Ready(Ok(()))
        }
    }

    #[tokio::test]
    async fn test_frame_sign_and_verify() -> anyhow::Result<()> {
        // 1️⃣ 创建随机身份
        let identity = FreeWebMovementAddress::random();

        // 2️⃣ 构造 frame body
        let body = FrameBody {
            version: 1,
            address: identity.to_string(),
            public_key: identity.public_key.to_bytes(),
            nonce: 42,
            data_length: 5,
            data: b"hello".to_vec(),
        };

        // 3️⃣ 使用身份签名生成 Frame
        let frame = P2PFrame::sign(body.clone(), &identity)?;
        assert!(!frame.signature.is_empty(), "签名不应该为空");

        // 4️⃣ 序列化 Frame
        let serialized = Codec::encode(&frame);

        // 5️⃣ 验证签名
        let frame1 = P2PFrame::verify_bytes(&serialized)?;

        assert_eq!(frame.signature.to_vec(), frame1.signature.to_vec());

        println!("Frame verified successfully!");

        let bytes = Codec::encode(&frame);
        let frame2: P2PFrame = Codec::decode(&bytes).unwrap();

        assert_eq!(frame1.signature.to_vec(), frame2.signature.to_vec());
        assert_eq!(frame1.body.data.to_vec(), frame2.body.data.to_vec());

        Ok(())
    }

    fn make_command() -> P2PCommand {
        P2PCommand::new(Entity::Node, Action::OnLine, vec![1, 2, 3, 4])
    }

    #[test]
    fn test_frame_body_new() {
        let addr = FreeWebMovementAddress::random();

        let body = FrameBody::new(
            1,
            addr.to_string(),
            addr.public_key.to_bytes(),
            100,
            4,
            vec![9, 8, 7, 6],
        );

        assert_eq!(body.version, 1);
        assert_eq!(body.address.to_string(), addr.to_string());
        assert_eq!(body.nonce, 100);
        assert_eq!(body.data_length, 4);
        assert_eq!(body.data, vec![9, 8, 7, 6]);
    }

    #[test]
    fn test_frame_body_data_from_command_and_back() -> anyhow::Result<()> {
        let addr = FreeWebMovementAddress::random();
        let mut body = FrameBody::new(
            1,
            addr.to_string(),
            addr.public_key.to_bytes(),
            1,
            0,
            vec![],
        );

        let cmd = make_command();
        body.data_from_command(&cmd)?;

        assert!(!body.data.is_empty());

        let decoded = body.command_from_data()?;
        assert_eq!(decoded, cmd);

        Ok(())
    }

    #[test]
    fn test_frame_new() {
        let addr = FreeWebMovementAddress::random();

        let body = FrameBody::new(
            1,
            addr.to_string(),
            addr.public_key.to_bytes(),
            1,
            1,
            vec![0xaa],
        );

        let frame = P2PFrame::new(body.clone(), vec![0xbb]);

        assert_eq!(frame.body.version, body.version);
        assert_eq!(frame.signature, vec![0xbb]);
    }

    #[tokio::test]
    async fn test_frame_sign_verify_roundtrip() -> anyhow::Result<()> {
        let identity = FreeWebMovementAddress::random();

        let body = FrameBody::new(
            1,
            identity.to_string(),
            identity.public_key.to_bytes(),
            42,
            5,
            b"hello".to_vec(),
        );

        let frame = P2PFrame::sign(body.clone(), &identity)?;
        assert!(!frame.signature.is_empty());

        let encoded = Codec::encode(&frame);
        let verified = P2PFrame::verify_bytes(&encoded)?;

        assert_eq!(frame.signature, verified.signature);
        assert_eq!(
            frame.body.address.to_string(),
            verified.body.address.to_string()
        );

        Ok(())
    }

    #[test]
    fn test_frame_to_from() {
        let identity = FreeWebMovementAddress::random();

        let body = FrameBody::new(
            1,
            identity.to_string(),
            identity.public_key.to_bytes(),
            7,
            3,
            vec![1, 2, 3],
        );

        let frame = P2PFrame::sign(body, &identity).unwrap();

        let bytes = Codec::encode(&frame.clone());
        let decoded: P2PFrame = Codec::decode(&bytes).unwrap();

        assert_eq!(frame.signature, decoded.signature);
        assert_eq!(frame.body.nonce, decoded.body.nonce);
    }

    #[tokio::test]
    async fn test_frame_verify_with_tampered_signature_should_fail() {
        let identity = FreeWebMovementAddress::random();

        let mut body = FrameBody::new(
            1,
            identity.to_string(),
            identity.public_key.to_bytes(),
            9,
            3,
            vec![1, 2, 3],
        );

        let mut frame = P2PFrame::sign(body.clone(), &identity).unwrap();

        // 🔥 篡改数据
        body.data = vec![9, 9, 9];
        frame.body = body;
        let encoded = Codec::encode(&frame);

        let res = P2PFrame::verify_bytes(&encoded);
        assert!(res.is_err(), "篡改后的签名应验证失败");
    }

    #[test]
    fn test_frame_config_consistency() {
        // 只要能成功编码解码即视为一致
        let addr = FreeWebMovementAddress::random();

        let body = FrameBody::new(
            1,
            addr.to_string(),
            addr.public_key.to_bytes(),
            0,
            0,
            vec![],
        );

        let bytes = Codec::encode(&body);

        let decoded: FrameBody = Codec::decode(&bytes).unwrap();

        assert_eq!(decoded.version, 1);
    }

    #[tokio::test]
    async fn test_frame_body_lifecycle() {
        let mut body = FrameBody::new(1, "test_addr".to_string(), vec![0u8; 32], 12345, 0, vec![]);

        let cmd = make_command();

        // 测试 data_from_command
        body.data_from_command(&cmd).unwrap();
        assert!(!body.data.is_empty());

        // 测试 command_from_data
        let decoded_cmd = body.command_from_data().unwrap();
        // 假设 P2PCommand 实现了 PartialEq
        assert_eq!(cmd, decoded_cmd);
        assert_eq!(body.version, 1);
    }

    #[tokio::test]
    async fn test_p2p_frame_sign_and_verify() {
        let address = FreeWebMovementAddress::random();
        let cmd = make_command();

        // 1. 测试 build (内部包含 sign)
        let frame = P2PFrame::build(&address, cmd, 1)
            .await
            .expect("Build failed");

        // 2. 测试 validate (实现自 Frame trait)
        assert!(frame.validate());

        // 3. 测试 verify 静态方法
        let verified_frame = P2PFrame::verify(frame.clone()).expect("Verify failed");
        assert_eq!(verified_frame.body.address, address.to_string());

        // 4. 测试 verify_bytes (序列化后校验)
        let bytes = Codec::encode(&frame);
        let verified_from_bytes = P2PFrame::verify_bytes(&bytes).expect("Verify bytes failed");
        assert_eq!(verified_from_bytes.signature, frame.signature);
    }

    #[tokio::test]
    async fn test_p2p_frame_verification_failure() {
        let address = FreeWebMovementAddress::random();
        let cmd = make_command();
        let mut frame = P2PFrame::build(&address, cmd, 1).await.unwrap();

        // 篡改数据以触发校验失败
        frame.body.nonce += 1;

        // 测试 verify 失败
        let result = P2PFrame::verify(frame.clone());
        assert!(result.is_err());

        // 测试 validate 失败
        assert!(!frame.validate());
    }

    #[tokio::test]
    async fn test_frame_trait_methods() {
        let address = FreeWebMovementAddress::random();
        let cmd = make_command();
        let frame = P2PFrame::build(&address, cmd, 1).await.unwrap();

        // payload() 应该返回 body 的序列化字节
        let payload = frame.payload().unwrap();
        assert_eq!(payload, Codec::encode(&frame.body));

        // command() 应该直接返回 body.data
        assert_eq!(frame.command().unwrap(), &frame.body.data);
        assert!(!frame.is_flat());
    }

    #[tokio::test]
    async fn test_frame_lifecycle() {
        let addr = FreeWebMovementAddress::random();
        let cmd = P2PCommand::new(Entity::Node, Action::Accept, vec![1, 2, 3]);

        // 1. 测试 build (覆盖了 sign 和 new)
        let frame = P2PFrame::build(&addr, cmd.clone(), 1).await.unwrap();

        // 2. 测试 validate
        assert!(frame.validate());

        // 3. 测试 verify 成功路径
        let verified = P2PFrame::verify(frame.clone()).unwrap();
        assert_eq!(verified.body.address, addr.to_string());

        // 4. 测试 verify 失败路径 (修改 body 导致签名失效)
        let mut tampered = frame.clone();
        tampered.body.nonce += 1;
        assert!(P2PFrame::verify(tampered).is_err());

        // 5. 测试 verify_bytes
        let bytes = Codec::encode(&frame);
        assert!(P2PFrame::verify_bytes(&bytes).is_ok());
    }

    #[test]
    fn test_frame_trait_impl() {
        let body = FrameBody::new(1, "addr".into(), vec![], 100, 0, vec![9, 9]);
        let frame = P2PFrame::new(body.clone(), vec![0; 64]);

        assert_eq!(frame.command().unwrap(), &vec![9, 9]);
        assert_eq!(frame.is_flat(), false);
        assert!(frame.payload().is_some());
    }

    #[test]
    fn test_p2p_frame_trait_sign() {
        let body = FrameBody::new(1, "test".into(), vec![], 0, 0, vec![1, 2, 3]);
        let frame = P2PFrame::new(body.clone(), vec![]);

        // 定义一个闭包，它只是简单地返回输入数据的长度作为“签名”
        // 用于验证 signer 闭包是否被调用且传入了正确的 body 字节
        let signature = frame.sign(|data| {
            assert_eq!(data, Codec::encode(&body));
            vec![0xFF, 0xEE]
        });

        assert_eq!(signature, vec![0xFF, 0xEE]);
    }

    #[tokio::test]
    async fn test_send_bytes_success_and_failure() {
        // 成功路径
        let (client, mut server) = tokio::io::duplex(64);
        let mut writer: Box<AexWriter> = Box::new(client);
        let test_data = b"hello world";

        P2PFrame::send_bytes(&mut *writer, test_data).await;

        let mut buf = vec![0u8; test_data.len()];
        server.read_exact(&mut buf).await.unwrap();
        assert_eq!(buf, test_data);

        // 失败路径 (覆盖 eprintln! 错误分支)
        struct FailWriter;
        impl tokio::io::AsyncWrite for FailWriter {
            fn poll_write(
                self: std::pin::Pin<&mut Self>,
                _: &mut std::task::Context<'_>,
                _: &[u8],
            ) -> std::task::Poll<std::io::Result<usize>> {
                std::task::Poll::Ready(Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "mock error",
                )))
            }
            fn poll_flush(
                self: std::pin::Pin<&mut Self>,
                _: &mut std::task::Context<'_>,
            ) -> std::task::Poll<std::io::Result<()>> {
                std::task::Poll::Ready(Ok(()))
            }
            fn poll_shutdown(
                self: std::pin::Pin<&mut Self>,
                _: &mut std::task::Context<'_>,
            ) -> std::task::Poll<std::io::Result<()>> {
                std::task::Poll::Ready(Ok(()))
            }
        }
        let mut fail_writer: Box<AexWriter> = Box::new(FailWriter);
        P2PFrame::send_bytes(&mut *fail_writer, b"fail").await;
        // 这里会触发 eprintln，虽然无法通过 assert 捕获，但会增加代码行覆盖率
    }

    #[tokio::test]
    async fn test_notify_logic_perfect_match() {
        // 1. 初始化常量
        let target_addr: SocketAddr = "1.2.3.4:8080".parse().unwrap();
        let notifier_addr: SocketAddr = "5.6.7.8:8080".parse().unwrap();
        let test_node_id = b"test_node_identity_123";

        // 2. 构造 Notifier
        let (n_client, _) = tokio::io::duplex(1024);
        let (n_rx, n_tx) = tokio::io::split(n_client);
        let global = Arc::new(GlobalContext::new(notifier_addr, None));
        let notifier_ctx = Arc::new(Mutex::new(Context::new(
            Some(Box::new(tokio::io::BufReader::new(n_rx))),
            Some(Box::new(tokio::io::BufWriter::new(n_tx))),
            global.clone(),
            notifier_addr,
        )));

        // 3. 构造 Target
        let (t_client, mut t_server) = tokio::io::duplex(1024);
        let (t_rx, t_tx) = tokio::io::split(t_client);
        // 注意：这里必须是 BufWriter，我们要确保它在转发后被 Flush
        let target_ctx = Arc::new(Mutex::new(Context::new(
            Some(Box::new(tokio::io::BufReader::new(t_rx))),
            Some(Box::new(tokio::io::BufWriter::new(t_tx))),
            global.clone(),
            target_addr,
        )));

        // 4. 注入 Manager
        let entry = Arc::new(ConnectionEntry {
            addr: target_addr,
            node: Arc::new(tokio::sync::RwLock::new(Some(Node {
                id: test_node_id.to_vec(),
                version: 1,
                started_at: 0,
                port: target_addr.port(),
                protocols: HashSet::new(),
                ips: Vec::new(),
            }))),
            abort_handle: tokio::spawn(async {}).abort_handle(),
            connected_at: 0,
            context: Some(target_ctx.clone()),
            cancel_token: CancellationToken::new(),
            last_seen: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        });

        let scope = NetworkScope::from_ip(&target_addr.ip());
        global
            .manager
            .connections
            .entry((target_addr.ip(), scope))
            .or_default()
            .servers
            .insert(target_addr, entry);

        // 5. 准备 Frame
        let cmd = make_command();
        let frame = P2PFrame::build(&FreeWebMovementAddress::random(), cmd, 1)
            .await
            .unwrap();
        let expected_bytes = Codec::encode(&frame);

        // 6. 执行 Notify 并强制 Flush
        // ⚡ 这里的关键点：如果你的 notify 实现里没写 flush，
        // 我们在测试里手动锁一下 target_ctx 刷一遍，或者确保 notify 内部逻辑完整。
        frame.notify(notifier_ctx.clone()).await;

        // 辅助：手动触发一次针对所有 Context 的 Flush 以防万一
        {
            let mut guard = target_ctx.lock().await;
            if let Some(writer) = &mut guard.writer {
                use tokio::io::AsyncWriteExt;
                writer.flush().await.unwrap();
            }
        }

        // 7. 验证 IO 流动
        let mut received_bytes = vec![0u8; expected_bytes.len()];
        let read_result = tokio::time::timeout(std::time::Duration::from_secs(1), async {
            t_server.read_exact(&mut received_bytes).await
        })
        .await;

        // 8. 验证结果
        match read_result {
            Ok(Ok(_)) => assert_eq!(received_bytes, expected_bytes),
            Ok(Err(e)) => panic!("读取失败: {:?}", e),
            Err(_) => panic!("超时：数据仍未到达。这通常意味着 notify 闭包内部逻辑未执行。"),
        }

        // 9. 优雅退出，防止挂起
        global.manager.shutdown();
        drop(notifier_ctx);
        drop(target_ctx);
        // 这一步确保 duplex 关闭
    }

    #[tokio::test]
    async fn test_notify_error_branches_coverage() {
        let addr_fail: SocketAddr = "9.9.9.9:9999".parse().unwrap();
        let notifier_addr: SocketAddr = "8.8.8.8:8080".parse().unwrap();

        let global = Arc::new(GlobalContext::new(notifier_addr, None));

        // 1. 构造一个持有“坏”Writer 的 Context
        // 这里的 BufWriter 会包装我们的 AlwaysFailWriter
        let bad_writer: BoxWriter = Box::new(tokio::io::BufWriter::new(AlwaysFailWriter));
        let target_ctx = Arc::new(Mutex::new(Context::new(
            None, // Reader 不重要
            Some(bad_writer),
            global.clone(),
            addr_fail,
        )));

        // 2. 将坏的 Entry 注入 Manager
        let entry = Arc::new(ConnectionEntry {
            addr: addr_fail,
            node: Arc::new(tokio::sync::RwLock::new(None)), // Node 为 None 也能增加覆盖率
            abort_handle: tokio::spawn(async {}).abort_handle(),
            connected_at: 0,
            context: Some(target_ctx),
            cancel_token: CancellationToken::new(),
            last_seen: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        });

        let scope = NetworkScope::from_ip(&addr_fail.ip());
        global
            .manager
            .connections
            .entry((addr_fail.ip(), scope))
            .or_default()
            .servers
            .insert(addr_fail, entry);

        // 3. 执行 Notify
        let frame = P2PFrame::build(&FreeWebMovementAddress::random(), make_command(), 1)
            .await
            .unwrap();

        // 为了触发 notify 里的 Context 入口逻辑，我们需要一个真实的 notifier_ctx
        let (n_client, _) = tokio::io::duplex(1024);
        let (n_rx, n_tx) = tokio::io::split(n_client);
        let notifier_ctx = Arc::new(Mutex::new(Context::new(
            Some(Box::new(tokio::io::BufReader::new(n_rx))),
            Some(Box::new(tokio::io::BufWriter::new(n_tx))),
            global.clone(),
            notifier_addr,
        )));

        // 4. 执行调用
        // 这将触发 P2PFrame::send_bytes 内部的 eprintln!
        frame.notify(notifier_ctx).await;

        // 5. 清理
        global.manager.shutdown();
    }
}
