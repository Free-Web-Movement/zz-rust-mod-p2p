use std::{ collections::HashMap, sync::Arc };
use regex::Regex;
use tokio::sync::Mutex;

use crate::http::{
    handler::{ Executor, HTTPContext, Handler },
    params::Params,
    protocol::method::HttpMethod,
};

/// 路由条目
pub struct RouteEntry {
    pub regex: Regex, // 匹配正则
    pub raw_path: String, // 原始路径
    pub handler: Handler, // 处理器
    pub param_names: Vec<String>, // 路径参数名
}

/// Router
pub struct Router {
    pub routes: Vec<RouteEntry>,
}

impl Router {
    /// 创建 Router
    pub fn new() -> Self {
        Self { routes: vec![] }
    }

    /// 注册路由
    pub fn add(
        &mut self,
        paths: Vec<&str>,
        methods: Vec<&str>,
        executors: Vec<Executor>
    ) -> &mut Self {
        for path in paths {
            let (regex_str, param_names) = Params::parse_path_regex(path);
            let re = Regex::new(&regex_str).unwrap();

            if let Some(entry) = self.routes.iter_mut().find(|r| r.raw_path == path) {
                for method in &methods {
                    let m = HttpMethod::from_str(method).unwrap();
                    entry.handler.add_vec(&mut param_names.clone(), Some(m), executors.clone());
                }
            } else {
                let mut handler = Handler::new();
                for method in &methods {
                    let m = HttpMethod::from_str(method).unwrap();
                    handler.add_vec(&mut param_names.clone(), Some(m), executors.clone());
                }
                self.routes.push(RouteEntry {
                    regex: re,
                    raw_path: path.to_string(),
                    handler,
                    param_names,
                });
            }
        }
        self
    }

    /// 快捷注册方法
    pub fn get(&mut self, paths: Vec<&str>, executors: Vec<Executor>) -> &mut Self {
        self.add(paths, vec!["GET"], executors)
    }
    pub fn post(&mut self, paths: Vec<&str>, executors: Vec<Executor>) -> &mut Self {
        self.add(paths, vec!["POST"], executors)
    }
    pub fn put(&mut self, paths: Vec<&str>, executors: Vec<Executor>) -> &mut Self {
        self.add(paths, vec!["PUT"], executors)
    }
    pub fn delete(&mut self, paths: Vec<&str>, executors: Vec<Executor>) -> &mut Self {
        self.add(paths, vec!["DELETE"], executors)
    }
    pub fn patch(&mut self, paths: Vec<&str>, executors: Vec<Executor>) -> &mut Self {
        self.add(paths, vec!["PATCH"], executors)
    }
    pub fn options(&mut self, paths: Vec<&str>, executors: Vec<Executor>) -> &mut Self {
        self.add(paths, vec!["OPTIONS"], executors)
    }
    pub fn head(&mut self, paths: Vec<&str>, executors: Vec<Executor>) -> &mut Self {
        self.add(paths, vec!["HEAD"], executors)
    }
    pub fn trace(&mut self, paths: Vec<&str>, executors: Vec<Executor>) -> &mut Self {
        self.add(paths, vec!["TRACE"], executors)
    }
    pub fn connect(&mut self, paths: Vec<&str>, executors: Vec<Executor>) -> &mut Self {
        self.add(paths, vec!["CONNECT"], executors)
    }

    pub async fn process(&mut self, context: HTTPContext) {
        // 包一层 Arc<Mutex<_>>，整个请求生命周期只用这一份
        let ctx = Arc::new(Mutex::new(context));

        // 先读取 path / method（只读，不跨 await）
        let (req_path, req_method) = {
            let ctx_guard = ctx.lock().await;
            let req = ctx_guard.req.lock().await;
            (req.path.clone(), req.method.clone())
        };

        for route in &mut self.routes {
            if let Some(caps) = route.regex.captures(&req_path) {
                // ---------- 填充 path 参数 ----------
                let mut path_params = HashMap::new();
                for (i, name) in route.param_names.iter().enumerate() {
                    if let Some(m) = caps.get(i + 1) {
                        path_params.insert(name.clone(), m.as_str().to_string());
                    }
                }

                {
                    let ctx_guard = ctx.lock().await;
                    let mut req = ctx_guard.req.lock().await;
                    req.params.path = Some(path_params);
                }

                // ---------- 取 executors（必须 clone，不能跨 await 持 borrow） ----------
                let executors: Vec<Executor> = route.handler
                    .get_executors(Some(&req_method))
                    .clone();

                // ---------- 串行执行 middleware ----------
                for exec in executors {
                    let continue_chain = exec(ctx.clone()).await;
                    if !continue_chain {
                        break;
                    }
                }

                return;
            }
        }
    }
}
