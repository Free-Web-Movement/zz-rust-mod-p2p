use aex::connection::global::GlobalContext;
use clap::Parser;
use futures::future::BoxFuture;
use std::{collections::HashMap, sync::Arc};
use tokio::io::AsyncBufReadExt;

use crate::clis::{connect, help, info, peers, send, status};

// 定义处理函数的类型：接收 Node 引用和剩余参数列表
pub type CliHandler =
    Box<dyn Fn(Vec<String>, Arc<GlobalContext>) -> BoxFuture<'static, ()> + Send + Sync>;

pub struct Cli {
    pub commands: HashMap<String, CliHandler>,
}

#[derive(Parser, Debug, Default)]
#[command(name = "zzp2p")]
pub struct Opt {
    #[arg(long, default_value = "zz-p2p-node")]
    pub name: String,

    #[arg(long, default_value = "0.0.0.0")]
    pub ip: String,

    #[arg(long, default_value_t = 9000)]
    pub port: u16,

    #[arg(long)]
    pub data_dir: Option<String>,

    #[arg(long)]
    pub address_file: Option<String>,

    #[arg(long)]
    pub inner_server_file: Option<String>,

    #[arg(long)]
    pub external_server_file: Option<String>,

    #[arg(long, default_value_t = false)]
    pub test: bool,
}

impl Cli {
    pub fn new() -> Self {
        let mut cli = Self {
            // node,
            commands: HashMap::new(),
        };
        cli.register_builtins();
        cli
    }

    /// 注册命令处理函数
    pub fn register<F, Fut>(&mut self, name: &str, handler: F)
    where
        // F 是一个函数，它接收 Node 和 Args，返回 Fut
        F: Fn(Vec<String>, Arc<GlobalContext>) -> Fut + Send + Sync + 'static,
        // Fut 是这个函数返回的异步任务
        Fut: Future<Output = ()> + Send + 'static,
    {
        // 在内部手动包装成 BoxFuture
        let boxed_handler = Box::new(move |args: Vec<String>, ctx: Arc<GlobalContext>| {
            let fut = handler(args, ctx);
            Box::pin(fut) as BoxFuture<'static, ()>
        });

        self.commands.insert(name.to_string(), boxed_handler);
    }

    fn register_builtins(&mut self) {
        // --- 注册 send 命令 ---
        self.register("send", send::handle);

        // --- 注册 connect 命令 ---
        self.register("connect", connect::handle);

        // --- 注册 status 命令 ---
        self.register("status", status::handle);

        // --- 注册 help 命令 ---
        self.register("help", help::handle);

        // --- 注册 peers 命令 ---
        self.register("peers", peers::handle);

        // --- 注册 info 命令 ---
        self.register("info", info::handle);
    }

    pub async fn run<R>(&self, reader: R, ctx: Arc<GlobalContext>) -> anyhow::Result<()>
    where
        R: tokio::io::AsyncBufRead + Unpin,
    {
        println!("Type 'help' for commands.");

        // let stdin = io::stdin();
        let mut lines = reader.lines();

        while let Some(line) = lines.next_line().await? {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let mut parts: Vec<String> = line.split_whitespace().map(|s| s.to_string()).collect();
            let command_name = parts.remove(0);

            if command_name == "exit" {
                println!("Exiting...");
                break;
            }

            if let Some(handler) = self.commands.get(&command_name) {
                // 执行注册的处理函数
                handler(parts, ctx.clone()).await;
            } else {
                println!("Unknown command: '{}', type 'help' for help", command_name);
            }
        }
        Ok(())
    }
}
