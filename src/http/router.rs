use std::{ collections::HashMap, vec };

use regex::Regex;
use serde_json::{ Value, Map };

use crate::http::{req::{Request, Response}, router_reg::generate_common_regex_str};

// pub type Scope = HashMap<String, ()>;

pub struct HTTPContext<'a> {
    pub req: &'a Request,
    pub res: &'a mut Response,
    pub global: &'a Map<String, Value>,
    pub local: Map<String, Value>,
}

// Executor is a middleware function returns bool
// 1. true to continue
// 2. false to stop further execution

pub type Executor = fn(context: &mut HTTPContext) -> bool;

pub type RouterPasser = fn() -> Router;

#[derive(Debug, Clone)]
pub struct HandlerMapValue {
    parameters: Vec<String>,
    executors: Vec<Executor>,
}

#[derive(Debug, Clone)]
pub struct Handler {
    pub methods: HashMap<String, HandlerMapValue>,

    // Default Executors for all methods
    pub fallback: HandlerMapValue,
}

#[derive(Debug, Clone)]
pub struct Router {
    pub map: HashMap<String, Handler>,
}

impl Handler {
    pub fn new() -> Self {
        Handler {
            methods: HashMap::new(),
            fallback: HandlerMapValue {
                parameters: vec![],
                executors: vec![],
            },
        }
    }

    pub fn add(
        &mut self,
        params: &mut Vec<String>,
        method: String,
        executor: Executor
    ) -> &mut Handler {
        if method.is_empty() {
        
            self.fallback.parameters.append(params);
            self.fallback.executors = vec![executor];
        } else {
            if self.methods.contains_key(&method) {
                let value = self.methods.get_mut(&method).unwrap();
                if !value.executors.contains(&executor) {
                    value.executors.push(executor);
                }
            } else {
                let mut new_executors = Vec::new();
                new_executors.push(executor);
                let value = HandlerMapValue {
                    parameters: params.clone(),
                    executors: new_executors,
                };
                self.methods.insert(method, value);
            }
        }
        self
    }

    pub fn add_vec(
        &mut self,
        params: &Vec<String>,
        method: String,
        mut executors: Vec<Executor>
    ) -> &mut Handler {
        if method.is_empty() {
            self.fallback.executors.append(&mut executors);
        } else {
            if self.methods.contains_key(&method) {
                let value = self.methods.get_mut(&method).unwrap();
                value.executors.append(&mut executors);
            } else {
                let value = HandlerMapValue {
                    parameters: params.clone(),
                    executors: executors,
                };
                self.methods.insert(method, value);
            }
        }
        self
    }
}

impl Router {
    pub fn new() -> Self {
        Router {
            map: HashMap::new(),
        }
    }

    pub fn add(
        &mut self,
        pathes: Vec<&str>,
        methods: Vec<&str>,
        executors: Vec<Executor>
    ) -> &mut Self {
        for path in pathes {
            let (extracted, mut params) = generate_common_regex_str(path);
            if !self.map.contains_key(extracted.as_str()) {
                let mut handler = Handler::new();
                let new_methods = methods.clone();
                for method in new_methods {
                    let new_executors = executors.clone();
                    handler.add_vec(
                        &mut params,
                        method.trim().to_uppercase().to_string(),
                        new_executors
                    );
                }
                self.map.insert(path.to_string(), handler);
            } else {
                let handler = self.map.get_mut(path).unwrap();
                let new_methods = methods.clone();
                for method in new_methods {
                    let new_executors = executors.clone();
                    handler.add_vec(&mut params, method.to_uppercase().to_string(), new_executors);
                }
            }
        }
        self
    }

    // add http method handle

    pub fn get(&mut self, pathes: Vec<&str>, executors: Vec<Executor>) -> &mut Self {
        self.add(pathes, vec!["GET"], executors)
    }

    pub fn post(&mut self, pathes: Vec<&str>, executors: Vec<Executor>) -> &mut Self {
        self.add(pathes, vec!["POST"], executors)
    }

    pub fn head(&mut self, pathes: Vec<&str>, executors: Vec<Executor>) -> &mut Self {
        self.add(pathes, vec!["HEAD"], executors)
    }

    pub fn options(&mut self, pathes: Vec<&str>, executors: Vec<Executor>) -> &mut Self {
        self.add(pathes, vec!["OPTIONS"], executors)
    }

    pub fn trace(&mut self, pathes: Vec<&str>, executors: Vec<Executor>) -> &mut Self {
        self.add(pathes, vec!["TRACE"], executors)
    }

    pub fn put(&mut self, pathes: Vec<&str>, executors: Vec<Executor>) -> &mut Self {
        self.add(pathes, vec!["PUT"], executors)
    }

    pub fn delete(&mut self, pathes: Vec<&str>, executors: Vec<Executor>) -> &mut Self {
        self.add(pathes, vec!["DELETE"], executors)
    }

    pub fn patch(&mut self, pathes: Vec<&str>, executors: Vec<Executor>) -> &mut Self {
        self.add(pathes, vec!["PATCH"], executors)
    }

    pub fn connect(&mut self, pathes: Vec<&str>, executors: Vec<Executor>) -> &mut Self {
        self.add(pathes, vec!["CONNECT"], executors)
    }

    pub async fn process(&mut self, context: &mut HTTPContext<'_>) {
        let path = &context.req.path;
        let method = &context.req.method;
        // println!("process path: {}, method: {}", path, method);

        for (key, value) in & mut self.map.iter_mut() {
            println!("{}", key);
            // }
            // if self.map.contains_key(path) {
            let re = Regex::new(&key).unwrap();
            if re.is_match(&path) {
                // println!("process contaions key!");
                println!("{} matched with {}", key, path);
                // If there are method-specific executors, append them
                let mut executors = &mut value.fallback.executors;
                if value.methods.contains_key(method) {
                    executors = &mut value.methods.get_mut(method).unwrap().executors;
                }
                // Now you can process the executors as needed
                for executor in executors {
                    let should_continue = executor(context);
                    if !should_continue {
                        break;
                    }
                }
                return;
            }
        }
    }
}
