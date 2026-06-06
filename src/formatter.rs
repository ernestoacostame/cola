use crate::formats::ParsedLine;
use crate::geoip::GeoResult;
use colored::*;
use regex::Regex;
use std::net::IpAddr;

pub struct Formatter {
    ip_regex: Regex,
}

impl Formatter {
    pub fn new() -> Self {
        Self {
            // A fallback regex to extract IPs from raw lines if parsing fails
            ip_regex: Regex::new(r"\b((?:\d{1,3}\.){3}\d{1,3})\b").unwrap(),
        }
    }

    /// Primary entry point to format a log line.
    /// If `parsed` is provided, prints a structured representation.
    /// Otherwise, prints the raw line with basic keyword highlighting.
    pub fn format_line(&self, parsed: Option<&ParsedLine>, raw_line: &str, geo: Option<&GeoResult>) -> String {
        match parsed {
            Some(line) => self.format_parsed(line, geo),
            None => self.format_raw(raw_line, geo),
        }
    }

    /// Extracts an IP from a raw unparsed line (for fallback geolocation)
    pub fn extract_ip_fallback(&self, line: &str) -> Option<IpAddr> {
        let caps = self.ip_regex.captures(line)?;
        caps.get(1)?.as_str().parse().ok()
    }

    /// Formats a parsed structured line beautifully
    fn format_parsed(&self, line: &ParsedLine, geo: Option<&GeoResult>) -> String {
        let flag = geo.map(|g| g.flag.as_str()).unwrap_or("🏳");
        let ip_str = line
            .ip
            .map(|ip| format!("{:<15}", ip.to_string()))
            .unwrap_or_else(|| "local          ".to_string());
        
        let ip_formatted = ip_str.cyan().bold();

        // Format timestamp
        let time_formatted = line
            .timestamp
            .as_ref()
            .map(|t| format_timestamp(t))
            .unwrap_or_else(|| "        ".to_string())
            .truecolor(128, 128, 128); // Dim gray

        // Format service name
        let service = line
            .service
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or("");

        if service == "nginx" || service == "apache" {
            // Web Log formatting: Flag | IP | Time | Method | Status | Path | User Agent
            let method = line.method.as_deref().unwrap_or("-");
            let method_formatted = match method {
                "GET" => "GET   ".green().bold(),
                "POST" => "POST  ".yellow().bold(),
                "PUT" => "PUT   ".blue().bold(),
                "DELETE" => "DELETE".red().bold(),
                "PATCH" => "PATCH ".magenta().bold(),
                "HEAD" => "HEAD  ".cyan().bold(),
                m => format!("{:<6}", m).normal(),
            };

            let status = line.status.as_deref().unwrap_or("-");
            let status_formatted = if let Ok(code) = status.parse::<u16>() {
                match code {
                    200..=299 => status.green().bold(),
                    300..=399 => status.blue(),
                    400..=499 => status.yellow().bold(),
                    500..=599 => status.red().bold(),
                    _ => status.normal(),
                }
            } else {
                status.normal()
            };

            let path = line.path_or_msg.as_deref().unwrap_or("-");
            let path_formatted = path.white().bold();

            let user_agent = line.user_agent.as_deref().unwrap_or("-");
            // Truncate user agent to 35 chars
            let ua_formatted = if user_agent.len() > 35 {
                format!("{}...", &user_agent[..32]).truecolor(140, 140, 140)
            } else {
                user_agent.truecolor(140, 140, 140)
            };

            format!(
                "{} {} │ {} │ {} │ {} │ {} │ {}",
                flag, ip_formatted, time_formatted, method_formatted, status_formatted, path_formatted, ua_formatted
            )
        } else {
            // Syslog/SSH formatting: Flag | IP | Time | Service | Action | Message
            let service_formatted = format!("{:<6}", service).magenta().bold();

            let action = line.method.as_deref().unwrap_or("-");
            let action_formatted = match action {
                "ACCEPTED" | "OPENED" => "OK  ".green().bold(),
                "FAILED" | "INVALID" => "FAIL".red().bold(),
                "DISCONN" | "CLOSED" => "INFO".blue(),
                act => format!("{:<4}", act).normal(),
            };

            // Highlight any user name if detected in status field
            let mut message = line.path_or_msg.clone().unwrap_or_default();
            if let Some(ref user) = line.status {
                // Highlight user name in message
                let highlighted_user = user.yellow().bold().to_string();
                message = message.replace(user, &highlighted_user);
            }

            // Apply light keyword coloring to the syslog message body
            let message_colored = highlight_keywords(&message);

            format!(
                "{} {} │ {} │ {} │ {} │ {}",
                flag, ip_formatted, time_formatted, service_formatted, action_formatted, message_colored
            )
        }
    }

    /// Formats a raw unparsed line, highlighting keywords and adding flag if IP is found
    fn format_raw(&self, line: &str, geo: Option<&GeoResult>) -> String {
        let flag = geo.map(|g| g.flag.as_str()).unwrap_or("🏳");
        
        let ip_prefix = if let Some(g) = geo {
            format!("{} {} │ ", g.flag, g.country_code.cyan().bold())
        } else {
            format!("{}      │ ", flag)
        };

        let colored_line = highlight_keywords(line);
        format!("{}{}", ip_prefix, colored_line)
    }
}

/// Parse timestamp from log format and return formatted local time or date
fn format_timestamp(ts: &str) -> String {
    // E.g. "06/Jun/2026:12:34:56 +0000" or "Jun  6 12:34:56"
    // Let's extract just the HH:MM:SS or the date + time part for display
    if ts.contains(':') {
        let parts: Vec<&str> = ts.split(':').collect();
        if parts.len() >= 4 {
            // Nginx/Apache style: "06/Jun/2026:12:34:56 +0000"
            return format!("{}:{}:{}", parts[1], parts[2], parts[3].split_whitespace().next().unwrap_or(""));
        } else if parts.len() == 3 {
            // Syslog style: "Jun  6 12:36:00"
            if let Some(hour) = parts[0].split_whitespace().last() {
                return format!("{}:{}:{}", hour, parts[1], parts[2]);
            }
        }
    }
    
    // Fallback: return first 15 chars (useful for syslog "Jun  6 12:34:56")
    if ts.len() > 15 {
        ts[..15].to_string()
    } else {
        ts.to_string()
    }
}

/// Highlight common log keywords (FAIL, SUCCESS, Accepted, etc.)
fn highlight_keywords(text: &str) -> String {
    let mut result = text.to_string();

    // List of replacement pairs: (word, colored_word)
    // We do exact case-sensitive word replacements
    let keywords = [
        ("FAIL", "FAIL".red().bold().to_string()),
        ("FAILED", "FAILED".red().bold().to_string()),
        ("Failed", "Failed".red().bold().to_string()),
        ("ERROR", "ERROR".red().bold().to_string()),
        ("Error", "Error".red().bold().to_string()),
        ("error", "error".red().bold().to_string()),
        ("SUCCESS", "SUCCESS".green().bold().to_string()),
        ("SUCCESSFUL", "SUCCESSFUL".green().bold().to_string()),
        ("Accepted", "Accepted".green().bold().to_string()),
        ("accepted", "accepted".green().bold().to_string()),
        ("Invalid user", "Invalid user".red().bold().to_string()),
        ("invalid user", "invalid user".red().bold().to_string()),
        ("WARNING", "WARNING".yellow().bold().to_string()),
        ("Warning", "Warning".yellow().bold().to_string()),
    ];

    for &(word, ref colored_word) in &keywords {
        if result.contains(word) {
            result = result.replace(word, colored_word);
        }
    }

    result
}
