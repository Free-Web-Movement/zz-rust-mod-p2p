use futures::future::BoxFuture;
use std::{collections::HashMap, sync::Arc};
use tokio::{
    io::{self, AsyncBufReadExt, BufReader},
    sync::Mutex,
};

use crate::{
    clis::{connect, help, send, status},
    node::Node,
};

// 定义处理函数的类型：接收 Node 引用和剩余参数列表
pub type CliHandler =
    Box<dyn Fn(Arc<Mutex<Node>>, Vec<String>) -> BoxFuture<'static, ()> + Send + Sync>;

pub struct Cli {
    node: Arc<Mutex<Node>>,
    commands: HashMap<String, CliHandler>,
}

impl Cli {
    pub fn new(node: Arc<Mutex<Node>>) -> Self {
        let mut cli = Self {
            node,
            commands: HashMap::new(),
        };
        cli.register_builtins();
        cli
    }

    /// 注册命令处理函数
    pub fn register<F, Fut>(&mut self, name: &str, handler: F)
    where
        // F 是一个函数，它接收 Node 和 Args，返回 Fut
        F: Fn(Arc<Mutex<Node>>, Vec<String>) -> Fut + Send + Sync + 'static,
        // Fut 是这个函数返回的异步任务
        Fut: Future<Output = ()> + Send + 'static,
    {
        // 在内部手动包装成 BoxFuture
        let boxed_handler = Box::new(move |node: Arc<Mutex<Node>>, args: Vec<String>| {
            let fut = handler(node, args);
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
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        println!("Type 'help' for commands.");

        let stdin = io::stdin();
        let mut reader = BufReader::new(stdin).lines();

        while let Some(line) = reader.next_line().await? {
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
                handler(self.node.clone(), parts).await;
            } else {
                println!("Unknown command: '{}', type 'help' for help", command_name);
            }
        }
        Ok(())
    }
}
