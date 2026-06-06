use crate::formats::{LogParser, ParsedLine};
use regex::Regex;
use std::net::IpAddr;

pub struct SyslogParser {
    syslog_regex: Regex,
    ip_regex: Regex,
}

impl SyslogParser {
    pub fn new() -> Self {
        // Regex for Syslog Format:
        // E.g.: Jun  6 13:00:00 hostname sshd[12345]: Failed password for root from 192.168.1.100 port 54321 ssh2
        let syslog_regex = Regex::new(
            r#"(?x)
            ^([A-Z][a-z]{2}\s+\d+\s+\d{2}:\d{2}:\d{2})\s+  # 1. Timestamp (e.g. Jun  6 12:34:56)
            (\S+)\s+                                      # 2. Hostname
            ([a-zA-Z0-9_-]+)(?:\[\d+\])?:\s+              # 3. Service (e.g. sshd)
            (.*)$                                         # 4. Message
            "#
        ).unwrap();

        // Basic IPv4 extraction regex
        let ip_regex = Regex::new(r"\b((?:\d{1,3}\.){3}\d{1,3})\b").unwrap();

        Self {
            syslog_regex,
            ip_regex,
        }
    }
}

impl LogParser for SyslogParser {
    fn parse(&self, line: &str) -> Option<ParsedLine> {
        let caps = self.syslog_regex.captures(line)?;

        let timestamp = caps.get(1).map(|m| m.as_str().to_string());
        let service = caps.get(3).map(|m| m.as_str().to_string());
        let message = caps.get(4)?.as_str().to_string();

        // Scan message for IP addresses
        let mut ip: Option<IpAddr> = None;
        if let Some(ip_caps) = self.ip_regex.captures(&message) {
            if let Some(ip_match) = ip_caps.get(1) {
                if let Ok(parsed_ip) = ip_match.as_str().parse::<IpAddr>() {
                    ip = Some(parsed_ip);
                }
            }
        }

        // Determine "method" (action) and "status" (details/user) from message
        let mut method = None;
        let mut status = None;

        let lower_msg = message.to_lowercase();
        if lower_msg.contains("failed") {
            method = Some("FAILED".to_string());
        } else if lower_msg.contains("accepted") {
            method = Some("ACCEPTED".to_string());
        } else if lower_msg.contains("invalid user") {
            method = Some("INVALID".to_string());
        } else if lower_msg.contains("disconnect") {
            method = Some("DISCONN".to_string());
        } else if lower_msg.contains("session opened") {
            method = Some("OPENED".to_string());
        } else if lower_msg.contains("session closed") {
            method = Some("CLOSED".to_string());
        }

        // Try to extract user name
        // E.g. "for root", "for invalid user admin", "user=root"
        if lower_msg.contains("for ") {
            if let Some(idx) = lower_msg.find("for ") {
                let parts: Vec<&str> = message[idx + 4..].split_whitespace().collect();
                if !parts.is_empty() {
                    if parts[0] == "invalid" && parts.len() > 2 && parts[1] == "user" {
                        status = Some(parts[2].to_string());
                    } else {
                        status = Some(parts[0].to_string());
                    }
                }
            }
        } else if lower_msg.contains("user=") {
            if let Some(idx) = lower_msg.find("user=") {
                let parts: Vec<&str> = message[idx + 5..].split_whitespace().collect();
                if !parts.is_empty() {
                    // strip trailing comma if present
                    status = Some(parts[0].trim_end_matches(',').to_string());
                }
            }
        }

        Some(ParsedLine {
            timestamp,
            ip,
            service,
            method,
            path_or_msg: Some(message),
            status,
            user_agent: None,
            raw: line.to_string(),
        })
    }

    fn name(&self) -> &'static str {
        "syslog"
    }
}
