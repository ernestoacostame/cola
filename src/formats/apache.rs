use crate::formats::{LogParser, ParsedLine};
use regex::Regex;
use std::net::IpAddr;

pub struct ApacheParser {
    regex: Regex,
}

impl ApacheParser {
    pub fn new() -> Self {
        // Regex for Apache Log Format (supports optional VirtualHost prefix)
        // E.g.: 127.0.0.1 - - [06/Jun/2026:12:34:56 -0500] "GET /index.html HTTP/1.1" 200 2326
        // E.g. with VHost: example.com:80 127.0.0.1 - - [06/Jun/2026:12:34:56 -0500] "GET /index.html HTTP/1.1" 200 2326
        let regex = Regex::new(
            r#"(?x)
            ^(?:[a-zA-Z0-9.-]+:\d+\s+)?     # Optional VirtualHost (domain:port)
            (\S+)                          # 1. remote_addr (IP)
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

impl LogParser for ApacheParser {
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
            service: Some("apache".to_string()),
            method,
            path_or_msg: path,
            status,
            user_agent,
            raw: line.to_string(),
        })
    }

    fn name(&self) -> &'static str {
        "apache"
    }
}
