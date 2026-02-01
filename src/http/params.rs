use std::collections::HashMap;

use regex::Regex;

// 支持 :param? 可选参数 和 * 通配符
const PATH_PARAMS: &str = r"(?s)(?::([^/\.?]+)\??)|(\*)";
/// URL 参数结构
#[derive(Debug, Clone)]
pub struct Params {
    /// 原始请求 URL，包括 query
    pub url: String,
    /// Path 参数，例如 /user/:id -> {"id": "123"}
    pub path: HashMap<String, String>,
    /// Query 参数，例如 ?active=true -> {"active": "true"}
    pub query: HashMap<String, String>,
}

impl Params {
    pub fn new(url: String) -> Self {
        let path = HashMap::new();
        let query = Self::parse_query(&url);
        Self { url, path, query }
    }

    /// 根据 URL 提取 query params
    fn parse_query(url: &str) -> HashMap<String, String> {
        let mut map = HashMap::new();
        if let Some(pos) = url.find('?') {
            let query_str = &url[pos + 1..];
            for pair in query_str.split('&') {
                let mut kv = pair.splitn(2, '=');
                if let (Some(k), Some(v)) = (kv.next(), kv.next()) {
                    map.insert(k.to_string(), v.to_string());
                }
            }
        }
        map
    }

    /// 将 path pattern 转为正则并提取变量名
    ///
    /// Examples:
    /// "/user/:id/profile" => regex: "/user/([^/]+)/profile", params: ["id"]
    /// "/file/:name.:ext"   => regex: "/file/([^/]+)\\.([^/]+)", params: ["name","ext"]
    /// "/static/*"          => regex: "/static/(.*)", params: ["*"]
    pub fn parse_path_regex(path: &str) -> (String, Vec<String>) {
        let mut regex_str = String::new();
        let mut param_names = Vec::new();
        let mut pos = 0;
        let re = Regex::new(PATH_PARAMS).unwrap();

        for caps in re.captures_iter(path) {
            let whole = caps.get(0).unwrap();
            let path_s = &path[pos..whole.start()];
            regex_str += &regex::escape(path_s);

            if let Some(star) = caps.get(2) {
                // '*' 通配符
                regex_str += "(.*)";
                param_names.push("*".to_string());
            } else if let Some(name) = caps.get(1) {
                let name_str = name.as_str();
                print!("name_str = {}, whole = {}\n", name_str, whole.as_str());
                if whole.as_str().ends_with('?') {
                    // 可选参数
                    // ⚠️ 修改点：捕获组外层加非捕获组包裹 /? 保证索引安全
                    regex_str += "(?:/([^/]+))?";
                    println!("regex_str = {}", regex_str);
                } else {
                    regex_str += "([^/]+)";
                }
                param_names.push(name_str.to_string());
            }

            pos = whole.end();
        }

        // 剩余路径
        regex_str += &regex::escape(&path[pos..]);

        // ⚠️ 全匹配
        regex_str = format!("^{}$", regex_str);

        println!("regext_str = {}", regex_str);
        println!("param_names = {}", param_names.join(","));
        (regex_str, param_names)
    }

    /// 将 url 按正则 pattern 解析 path params
    pub fn extract_path_params(url: &str, pattern: &str) -> Option<HashMap<String, String>> {
        let (regex_str, param_names) = Self::parse_path_regex(pattern);

        println!("url = {}", url);
        println!("param_names = {}", param_names.join(","));

        let re = Regex::new(&regex_str).ok()?;
        let mut map = HashMap::new();
        let caps = re.captures(url)?;
        for (i, name) in param_names.iter().enumerate() {
            println!("name = {}", name);
            println!("i = {}", name);
            let mut v = "";
            if let Some(m) = caps.get(i + 1) {
                println!("m = {}", m.as_str());
                v = m.as_str();
            }
            map.insert(name.clone(), v.to_string());
        }
        Some(map)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_path() {
        let url = "/user/123/profile";
        let pattern = "/user/:id/profile";
        let params = Params::extract_path_params(url, pattern).unwrap();
        assert_eq!(params.get("id").unwrap(), "123");
    }

    #[test]
    fn test_star_path() {
        let url = "/static/css/main.css";
        let pattern = "/static/*";
        let params = Params::extract_path_params(url, pattern).unwrap();
        assert_eq!(params.get("*").unwrap(), "css/main.css");
    }

    #[test]
    fn test_optional_param() {
        let url = "/user/";
        let pattern = "/user/:id?";
        let params = Params::extract_path_params(url, pattern).unwrap();
        assert_eq!(params.get("id").unwrap(), "");
    }

    #[test]
    #[should_panic(expected = "called `Option::unwrap()` on a `None` value")]
    fn test_optional_param_should_panic() {
        Params::extract_path_params("/user", "/user/:id?").unwrap();
    }

    #[test]
    fn test_ext_param() {
        let url = "/file/report.pdf";
        let pattern = "/file/:name.:ext";
        let params = Params::extract_path_params(url, pattern).unwrap();
        assert_eq!(params.get("name").unwrap(), "report");
        assert_eq!(params.get("ext").unwrap(), "pdf");
    }
}
