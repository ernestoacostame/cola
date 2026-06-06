use crate::formats::{LogParser, ParsedLine};
use regex::Regex;
use std::net::IpAddr;

pub struct NginxParser {
    regex: Regex,
}

impl NginxParser {
    pub fn new() -> Self {
        // Regex for Nginx Combined Log Format:
        // $remote_addr - $remote_user [$time_local] "$request" $status $body_bytes_sent "$http_referer" "$http_user_agent"
        // E.g.: 192.168.1.1 - - [06/Jun/2026:12:34:56 +0000] "GET /index.html HTTP/1.1" 200 3426 "-" "Mozilla/5.0..."
        let regex = Regex::new(
            r#"(?x)
            ^(\S+)                          # 1. remote_addr (IP)
            \s+\S+\s+\S+\s+                 # remote_user / ident
            \[([^\]]+)\]\s+                 # 2. time_local
            "(\S+)\s+([^\s"]+)[^"]*"\s+     # 3. method, 4. path
            (\d{3})\s+                      # 5. status
            \d+(?:\s+"[^"]*"\s+"([^"]*)")?  # 6. user_agent (optional)
            "#
        ).unwrap();

        Self { regex }
    }
}

impl LogParser for NginxParser {
    fn parse(&self, line: &str) -> Option<ParsedLine> {
        let caps = self.regex.captures(line)?;

        let ip_str = caps.get(1)?.as_str();
        let ip: Option<IpAddr> = ip_str.parse().ok();

        let timestamp = caps.get(2).map(|m| m.as_str().to_string());
        let method = caps.get(3).map(|m| m.as_str().to_string());
        let path = caps.get(4).map(|m| m.as_str().to_string());
        let status = caps.get(5).map(|m| m.as_str().to_string());
        let user_agent = caps.get(6).map(|m| m.as_str().to_string());

        Some(ParsedLine {
            timestamp,
            ip,
            service: Some("nginx".to_string()),
            method,
            path_or_msg: path,
            status,
            user_agent,
            raw: line.to_string(),
        })
    }

    fn name(&self) -> &'static str {
        "nginx"
    }
}
