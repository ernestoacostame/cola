use std::net::IpAddr;

pub mod apache;
pub mod nginx;
pub mod syslog;

#[derive(Clone, Debug)]
pub struct ParsedLine {
    pub timestamp: Option<String>,
    pub ip: Option<IpAddr>,
    pub service: Option<String>,     // E.g., "nginx", "sshd"
    pub method: Option<String>,      // HTTP method (GET/POST) or Syslog category (Accepted/Failed)
    pub path_or_msg: Option<String>, // HTTP request path or SSH log message
    pub status: Option<String>,      // HTTP status code (200) or Syslog details
    pub user_agent: Option<String>,
    #[allow(dead_code)]
    pub raw: String,                 // The original unmodified log line
}

pub trait LogParser: Send + Sync {
    /// Try to parse a log line. Returns `Some(ParsedLine)` if it matches the format, or `None` if it does not.
    fn parse(&self, line: &str) -> Option<ParsedLine>;
    
    /// Returns the name of the parser format
    fn name(&self) -> &'static str;
}
