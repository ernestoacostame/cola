use crate::formats::{LogParser, ParsedLine};
use regex::Regex;
use std::net::IpAddr;

pub struct NginxParser {
    regex: Regex,
    error_regex: Regex,
    ip_regex: Regex,
    req_regex: Regex,
    host_regex: Regex,
    server_regex: Regex,
    upstream_regex: Regex,
    referrer_regex: Regex,
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

        // Regex for Nginx Error Log Format:
        // E.g.: 2026/06/06 05:45:48 [error] 22528#22528: *62981 access forbidden by rule, client: 35.77.212.118, server: senalesusa.com, request: "GET /development/.env HTTP/1.1", host: "senalesusa.com"
        let error_regex = Regex::new(
            r#"^(?P<time>\d{4}/\d{2}/\d{2}\s+\d{2}:\d{2}:\d{2})\s+\[(?P<level>[a-z]+)\]\s+\d+#\d+:\s+(?:\*\d+\s+)?(?P<msg>.+)$"#
        ).unwrap();

        let ip_regex = Regex::new(r"client:\s+([a-fA-F0-9:.]+)").unwrap();
        let req_regex = Regex::new(r#"request:\s+"([^"]+)""#).unwrap();
        let host_regex = Regex::new(r#"host:\s+"([^"]+)""#).unwrap();
        let server_regex = Regex::new(r"server:\s+([^,]+)").unwrap();
        let upstream_regex = Regex::new(r"upstream:\s+([^,]+)").unwrap();
        let referrer_regex = Regex::new(r#"referrer:\s+"([^"]+)""#).unwrap();

        Self {
            regex,
            error_regex,
            ip_regex,
            req_regex,
            host_regex,
            server_regex,
            upstream_regex,
            referrer_regex,
        }
    }
}

impl LogParser for NginxParser {
    fn parse(&self, line: &str) -> Option<ParsedLine> {
        // 1. Try Nginx Combined (Access) Log format
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
                service: Some("nginx".to_string()),
                method,
                path_or_msg: path,
                status,
                user_agent,
                raw: line.to_string(),
            });
        }

        // 2. Try Nginx Error Log format
        if let Some(caps) = self.error_regex.captures(line) {
            let timestamp = caps.name("time").map(|m| m.as_str().to_string());
            let level = caps.name("level").map(|m| m.as_str().to_string());
            let msg_full = caps.name("msg")?.as_str();

            // Find where metadata key-value pairs start (e.g. client:, server:, request:)
            let mut end_idx = msg_full.len();
            for marker in &[", client:", ", server:", ", request:", ", host:", ", upstream:", ", referrer:"] {
                if let Some(idx) = msg_full.find(marker) {
                    if idx < end_idx {
                        end_idx = idx;
                    }
                }
            }
            let pure_msg = msg_full[..end_idx].trim().to_string();

            // Extract client IP
            let ip = self.ip_regex
                .captures(msg_full)
                .and_then(|c| c.get(1))
                .and_then(|m| m.as_str().parse::<IpAddr>().ok());

            // Extract request details
            let request = self.req_regex
                .captures(msg_full)
                .map(|c| c.get(1).unwrap().as_str().to_string());

            // Extract server, host, upstream, referrer to build a unified details string
            let mut details = Vec::new();
            if let Some(c) = self.host_regex.captures(msg_full) {
                details.push(format!("host: {}", c.get(1).unwrap().as_str()));
            } else if let Some(c) = self.server_regex.captures(msg_full) {
                details.push(format!("server: {}", c.get(1).unwrap().as_str()));
            }
            if let Some(c) = self.upstream_regex.captures(msg_full) {
                details.push(format!("upstream: {}", c.get(1).unwrap().as_str()));
            }
            if let Some(c) = self.referrer_regex.captures(msg_full) {
                details.push(format!("referrer: {}", c.get(1).unwrap().as_str()));
            }

            let details_str = if details.is_empty() {
                None
            } else {
                Some(details.join(", "))
            };

            return Some(ParsedLine {
                timestamp,
                ip,
                service: Some("nginx_error".to_string()),
                method: level.map(|l| l.to_uppercase()),
                path_or_msg: Some(pure_msg),
                status: request, // Store request (e.g. "GET /dev/.env") in status
                user_agent: details_str,
                raw: line.to_string(),
            });
        }

        None
    }

    fn name(&self) -> &'static str {
        "nginx"
    }
}
