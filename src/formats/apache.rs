use crate::formats::{LogParser, ParsedLine};
use regex::Regex;
use std::net::IpAddr;

pub struct ApacheParser {
    regex: Regex,
    error_regex: Regex,
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

        // Regex for Apache Error Log Format:
        // E.g.: [Sun Jun 06 05:45:48.123456 2026] [core:error] [pid 12345:tid 12345] [client 35.77.212.118:54321] AH00123: Access denied by server configuration: /var/www/html/.env
        let error_regex = Regex::new(
            r#"^\[(?P<time>[^\]]+)\]\s+\[(?:[^\]:]+:)?(?P<level>[a-zA-Z]+)\]\s+(?:\[pid\s+\d+(?::tid\s+\d+)?\]\s+)?(?:\[client\s+(?P<client>[a-fA-F0-9:.]+)(?::\d+)?\])?\s*(?P<msg>.+)$"#
        ).unwrap();

        Self { regex, error_regex }
    }
}

impl LogParser for ApacheParser {
    fn parse(&self, line: &str) -> Option<ParsedLine> {
        // 1. Try Apache Access Log format
        if let Some(caps) = self.regex.captures(line) {
            let ip_str = caps.get(1)?.as_str();
            let ip: Option<IpAddr> = ip_str.parse().ok();

            let timestamp = caps.get(2).map(|m| m.as_str().to_string());
            let method = caps.get(3).map(|m| m.as_str().to_string());
            let path = caps.get(4).map(|m| m.as_str().to_string());
            let status = caps.get(5).map(|m| m.as_str().to_string());
            let user_agent = caps.get(6).map(|m| m.as_str().to_string());

            return Some(ParsedLine {
                timestamp,
                ip,
                service: Some("apache".to_string()),
                method,
                path_or_msg: path,
                status,
                user_agent,
                raw: line.to_string(),
            });
        }

        // 2. Try Apache Error Log format
        if let Some(caps) = self.error_regex.captures(line) {
            let timestamp = caps.name("time").map(|m| m.as_str().to_string());
            let level = caps.name("level").map(|m| m.as_str().to_string());
            let client_ip = caps.name("client")
                .map(|m| m.as_str())
                .and_then(|s| {
                    if let Ok(ip) = s.parse::<IpAddr>() {
                        Some(ip)
                    } else if let Some(pos) = s.rfind(':') {
                        s[..pos].parse::<IpAddr>().ok()
                    } else {
                        None
                    }
                });
            let msg = caps.name("msg").map(|m| m.as_str().to_string());

            return Some(ParsedLine {
                timestamp,
                ip: client_ip,
                service: Some("apache_error".to_string()),
                method: level.map(|l| l.to_uppercase()),
                path_or_msg: msg,
                status: None,
                user_agent: None,
                raw: line.to_string(),
            });
        }

        None
    }

    fn name(&self) -> &'static str {
        "apache"
    }
}
