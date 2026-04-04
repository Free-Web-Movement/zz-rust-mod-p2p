#[cfg(test)]
mod tests {
    use aex::connection::global::GlobalContext;
    use clap::Parser;
    use std::{
        io::Cursor,
        net::SocketAddr,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };
    use zz_p2p::{
        cli::{Cli, Opt},
        clis::{connect, help, send, status},
    };

    // 辅助函数：创建一个 Mock 的 GlobalContext
    // 假设 GlobalContext 至少实现了 Default，或者你可以根据实际情况构造
    fn create_mock_ctx() -> Arc<GlobalContext> {
        // 如果 GlobalContext 没有 Default，请替换为实际的构造逻辑
        let addr = "127.0.0.1:1080".parse::<SocketAddr>().unwrap();
        Arc::new(GlobalContext::new(addr, None))
    }

    #[test]
    fn test_cli_initialization() {
        let cli = Cli::new();
        // 验证内置命令是否已注册
        assert!(cli.commands.contains_key("send"));
        assert!(cli.commands.contains_key("connect"));
        assert!(cli.commands.contains_key("status"));
        assert!(cli.commands.contains_key("help"));
    }

    #[tokio::test]
    async fn test_custom_registration_and_execution() {
        let mut cli = Cli::new();
        let call_count = Arc::new(AtomicUsize::new(0));
        let count_clone = call_count.clone();

        // 注册一个测试命令
        cli.register("test_cmd", move |args, _ctx| {
            let count = count_clone.clone();
            async move {
                assert_eq!(args[0], "hello");
                count.fetch_add(1, Ordering::SeqCst);
            }
        });

        let ctx = create_mock_ctx();
        let input = "test_cmd hello\nexit\n";
        let reader = Cursor::new(input);

        let result = cli.run(reader, ctx).await;

        assert!(result.is_ok());
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_run_branch_coverage() {
        let mut cli = Cli::new();
        let ctx = create_mock_ctx();

        // 记录调用次数以验证正确分支
        let help_called = Arc::new(AtomicUsize::new(0));
        let help_clone = help_called.clone();

        cli.register("help", move |_, _| {
            let c = help_clone.clone();
            async move {
                c.fetch_add(1, Ordering::SeqCst);
            }
        });

        // 构造覆盖所有分支的输入：
        // 1. "\n" -> 空行分支 (line.is_empty())
        // 2. "unknown arg" -> 未匹配分支 (else)
        // 3. "help" -> 匹配成功分支 (Some(handler))
        // 4. "exit" -> 退出分支 (command_name == "exit")
        let input = "\n   \nunknown_command\nhelp\nexit\nignored_after_exit\n";
        let reader = Cursor::new(input);

        let result = cli.run(reader, ctx).await;

        assert!(result.is_ok());
        assert_eq!(help_called.load(Ordering::SeqCst), 1);
        // 逻辑验证：如果 exit 成功 break，"ignored_after_exit" 不会被处理
    }

    #[test]
    fn test_opt_default_via_clap() {
        // 模拟命令行参数，只传程序名，不传任何参数
        // 这会触发 clap 的默认值注入逻辑
        let opt = Opt::parse_from(&["zzp2p"]);

        assert_eq!(opt.name, "zz-p2p-node");
        assert_eq!(opt.ip, "0.0.0.0");
        assert_eq!(opt.port, 9000);
        assert!(opt.data_dir.is_none());
        assert!(opt.address_file.is_none());
    }

    #[test]
    fn test_opt_custom_values() {
        // 测试手动输入参数的情况
        let opt = Opt::parse_from(&[
            "zzp2p",
            "--name",
            "custom-node",
            "--port",
            "8080",
            "--data-dir",
            "/tmp/data",
        ]);

        assert_eq!(opt.name, "custom-node");
        assert_eq!(opt.port, 8080);
        assert_eq!(opt.data_dir, Some("/tmp/data".to_string()));
    }

    #[tokio::test]
    async fn test_all_builtin_command_routing() {
        let mut cli = Cli::new();
        let ctx = create_mock_ctx();

        // 为了不触发真实的 send/connect 逻辑（可能导致 crash 或挂起），
        // 我们在测试中“覆盖”这些内置命令的实现。
        let counter = Arc::new(AtomicUsize::new(0));

        let commands = vec!["send", "connect", "status", "help"];

        for cmd in &commands {
            let c = counter.clone();
            cli.register(cmd, move |_args, _ctx| {
                let c = c.clone();
                async move {
                    c.fetch_add(1, Ordering::SeqCst);
                }
            });
        }

        // 构造输入：每一行触发一个命令，最后退出
        let input = "send target msg\nconnect 127.0.0.1:9000\nstatus\nhelp\nexit\n";
        let reader = Cursor::new(input);

        let result = cli.run(reader, ctx).await;

        assert!(result.is_ok());
        // 验证 4 个命令是否都被调用过
        assert_eq!(counter.load(Ordering::SeqCst), 4);
    }

    #[tokio::test]
    async fn test_unknown_command_and_empty_lines() {
        let cli = Cli::new();
        let ctx = create_mock_ctx();

        // 测试包含空行、空格行、未知命令
        let input = "\n  \n  unknown_cmd_xyz  \nexit\n";
        let reader = Cursor::new(input);

        let result = cli.run(reader, ctx.clone()).await;
        assert!(result.is_ok());

        let args = Vec::new();
        let context = ctx.clone();
        help::handle(args.clone(), context.clone()).await;
        status::handle(args.clone(), context.clone()).await;
        send::handle(args.clone(), context.clone()).await;
        connect::handle(args.clone(), context.clone()).await;
    }

    #[test]
    fn test_opt_parsing_logic() {
        // 覆盖 Opt 的所有参数解析分支
        let args = vec![
            "zzp2p",
            "--name",
            "test-node",
            "--ip",
            "1.1.1.1",
            "--port",
            "7777",
            "--data-dir",
            "/tmp/zz",
            "--address-file",
            "addr.json",
            "--inner-server-file",
            "inner.json",
            "--external-server-file",
            "ext.json",
        ];

        let opt = Opt::parse_from(args);

        assert_eq!(opt.name, "test-node");
        assert_eq!(opt.ip, "1.1.1.1");
        assert_eq!(opt.port, 7777);
        assert_eq!(opt.data_dir, Some("/tmp/zz".to_string()));
        assert_eq!(opt.address_file, Some("addr.json".to_string()));
        assert_eq!(opt.inner_server_file, Some("inner.json".to_string()));
        assert_eq!(opt.external_server_file, Some("ext.json".to_string()));
    }
}
