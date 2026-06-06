use clap::{Parser, ValueEnum};
use std::path::PathBuf;

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogFormat {
    Auto,
    Nginx,
    Apache,
    Syslog,
}

#[derive(Parser, Debug)]
#[command(
    name = "cola",
    author = "Cola Contributors",
    version = "0.1.0",
    about = "🥤 Cola — Real-time log monitoring with GeoIP flags, colored parsing and filtering",
    long_about = "A high-performance tail -f replacement that parses log files (Nginx, Apache, Syslog), geolocates IP addresses to flags, and filters/formats logs in real-time."
)]
pub struct Args {
    /// The log files to monitor in real-time
    #[arg(value_name = "FILES", required = true, num_args = 1..)]
    pub files: Vec<PathBuf>,

    /// Path to MaxMind GeoLite2-Country (.mmdb) database [default: ~/.cola/GeoLite2-Country.mmdb]
    #[arg(
        short = 'd',
        long = "db",
        value_name = "PATH"
    )]
    pub db_path: Option<PathBuf>,

    /// Log format to use (auto-detects by default)
    #[arg(short = 'f', long = "format", value_enum, default_value_t = LogFormat::Auto)]
    pub format: LogFormat,

    /// Include filter: only show lines matching this regex/keyword
    #[arg(short = 'i', long = "include", value_name = "REGEX")]
    pub include: Option<String>,

    /// Exclude filter: hide lines matching this regex/keyword
    #[arg(short = 'e', long = "exclude", value_name = "REGEX")]
    pub exclude: Option<String>,

    /// Disable GeoIP country resolution and flag printing
    #[arg(long = "no-geo")]
    pub no_geo: bool,

    /// Number of existing lines to print from the end of the file on startup
    #[arg(short = 'n', long = "tail", default_value = "10")]
    pub tail: usize,
}
