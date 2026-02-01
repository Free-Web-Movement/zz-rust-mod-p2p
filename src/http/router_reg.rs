// use std::collections::HashMap;

// /**
//  * Routing URL Patterns.
//  * Routing URL Patterns are used to make routing easier.
//  * Ideas drived from Path-to-RegExp in node.js.
//  * 1. Parameters
//  *   "/:name"
//  *
//  */

// use regex::Regex;

// const PATH_PARAMS: &str = r"(?s)(?::([^/\.]+))|(?:\*)";

// pub fn match_all(route_pattern: String, actual_path: String) -> bool {
//     if match_paratemers(route_pattern, actual_path) {
//         return true;
//     }
//     return false;
// }

// pub fn match_paratemers(route_pattern: String, actual_path: String) -> bool {
//     let re = Regex::new(r"((/)?[^\/]+)*").unwrap();
//     assert!(re.is_match("https://example.com"));
//     assert!(!re.is_match("example.com")); // Missing https
//     return true;
// }

// pub fn into_map(re: &Regex, text: &str) {
//     let caps = re.captures(text).unwrap();
//     println!("{:#?}", caps);

//     for i in caps.iter() {
//         println!("{:#?}", i);
//     }
//     let dict: HashMap<&str, &str> = re
//         .capture_names()
//         .flatten()
//         .filter_map(|n| {
//             println!("{}", n);
//             Some((n, caps.name(n)?.as_str()))
//         })
//         .collect();
//     println!("{:#?}", dict);
// }

// pub fn generate_common_regex_str(path: &str) -> (String, Vec<String>) {
//     let mut regex_str = String::with_capacity(path.len());
//     let mut param_names = Vec::new();

//     let mut pos: usize = 0;
//     let re = Regex::new(PATH_PARAMS).unwrap();

//     for caps in re.captures_iter(path) {
//         let whole = caps.get(0).unwrap();

//         let path_s = &path[pos..whole.start()];
//         regex_str += &regex::escape(path_s);

//         if whole.as_str() == "*" {
//             regex_str += r"(.*)";
//             param_names.push("*".to_owned());
//         } else {
//             regex_str += r"([^/]+)";
//             param_names.push(caps.get(1).unwrap().as_str().to_owned());
//         }

//         pos = whole.end();
//     }

//     let left_over_path_s = &path[pos..];
//     regex_str += &regex::escape(left_over_path_s);

//     (regex_str, param_names)
// }
