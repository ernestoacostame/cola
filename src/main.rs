mod cache;
mod cli;
mod formatter;
mod formats;
mod geoip;
mod parser;
mod watcher;

use cache::IpCache;
use clap::Parser;
use colored::*;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use formatter::Formatter;
use geoip::GeoIp;
use parser::ParserManager;
use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::watch;
use watcher::FileWatcher;

// Predefined interactive filters
#[derive(Clone, Debug, Default)]
struct InteractiveFilters {
    hide_static: bool,
    only_errors: bool,
    hide_bots: bool,
    only_sshd: bool,
}

struct Stats {
    total_lookups: AtomicUsize,
    cache_hits: AtomicUsize,
}

// A macro to print with carriage return in raw mode to prevent staircase effect
macro_rules! println_raw {
    ($($arg:tt)*) => {{
        print!($($arg)*);
        print!("\r\n");
        let _ = std::io::stdout().flush();
    }};
}

struct RawModeGuard;

impl RawModeGuard {
    fn new() -> Self {
        crossterm::terminal::enable_raw_mode().ok();
        Self
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        crossterm::terminal::disable_raw_mode().ok();
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. Parse CLI Arguments
    let args = cli::Args::parse();

    // 2. Initialize Geolocation & Cache
    let db_path = args.db_path.clone().unwrap_or_else(|| {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        std::path::PathBuf::from(home).join(".cola").join("GeoLite2-Country.mmdb")
    });
    let geoip = Arc::new(GeoIp::new(&db_path));
    let cache = Arc::new(IpCache::new());
    let stats = Arc::new(Stats {
        total_lookups: AtomicUsize::new(0),
        cache_hits: AtomicUsize::new(0),
    });

    // 3. Setup Parser Manager and Formatter
    let mut parser_manager = ParserManager::new(
        args.format,
        args.include.as_deref(),
        args.exclude.as_deref(),
    )?;
    let formatter = Formatter::new();

    // 4. Enable raw mode and start key listener task
    let _raw_guard = RawModeGuard::new();
    let (filters_tx, filters_rx) = watch::channel(InteractiveFilters::default());

    // 5. Print Startup Banner
    print_banner(&args, &geoip);
    println_raw!("💡 Press 'h' at any time to show the interactive filter helper menu.\n");

    // 6. Setup exit statistics handler
    let shutdown_stats = Arc::clone(&stats);
    let shutdown_cache = Arc::clone(&cache);
    let exit_handler = move || {
        crossterm::terminal::disable_raw_mode().ok();
        println_raw!("\n");
        println_raw!("{}", "🥤 Exiting Cola...".yellow().bold());
        
        let lookups = shutdown_stats.total_lookups.load(Ordering::Relaxed);
        let hits = shutdown_stats.cache_hits.load(Ordering::Relaxed);
        let size = shutdown_cache.size();
        
        if lookups > 0 {
            let hit_rate = (hits as f64 / lookups as f64) * 100.0;
            println_raw!(
                "📊 Cache Stats: Total Lookups: {}, Hits: {} ({:.1}%), Cache Size: {} IPs",
                lookups, hits, hit_rate, size
            );
        } else {
            println_raw!("📊 Cache Stats: No lookups performed.");
        }
        std::process::exit(0);
    };

    // Spawn key listener task
    let stats_exit = Arc::clone(&stats);
    let cache_exit = Arc::clone(&cache);
    tokio::spawn(async move {
        let mut current_filters = InteractiveFilters::default();
        loop {
            if event::poll(std::time::Duration::from_millis(100)).unwrap_or(false) {
                if let Ok(Event::Key(key)) = event::read() {
                    // Check Ctrl+C in raw mode
                    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                        // Clean exit
                        crossterm::terminal::disable_raw_mode().ok();
                        println_raw!("\n");
                        println_raw!("{}", "🥤 Exiting Cola...".yellow().bold());
                        let lookups = stats_exit.total_lookups.load(Ordering::Relaxed);
                        let hits = stats_exit.cache_hits.load(Ordering::Relaxed);
                        let size = cache_exit.size();
                        if lookups > 0 {
                            let hit_rate = (hits as f64 / lookups as f64) * 100.0;
                            println_raw!(
                                "📊 Cache Stats: Total Lookups: {}, Hits: {} ({:.1}%), Cache Size: {} IPs",
                                lookups, hits, hit_rate, size
                            );
                        } else {
                            println_raw!("📊 Cache Stats: No lookups performed.");
                        }
                        std::process::exit(0);
                    }

                    let mut changed = true;
                    let mut msg = String::new();
                    match key.code {
                        KeyCode::Char('1') => {
                            current_filters.hide_static = !current_filters.hide_static;
                            msg = format!("Hide Static Assets: {}", if current_filters.hide_static { "ON".green().bold() } else { "OFF".red().bold() });
                        }
                        KeyCode::Char('2') => {
                            current_filters.only_errors = !current_filters.only_errors;
                            msg = format!("Show Only Errors: {}", if current_filters.only_errors { "ON".green().bold() } else { "OFF".red().bold() });
                        }
                        KeyCode::Char('3') => {
                            current_filters.hide_bots = !current_filters.hide_bots;
                            msg = format!("Hide Bots/Crawlers: {}", if current_filters.hide_bots { "ON".green().bold() } else { "OFF".red().bold() });
                        }
                        KeyCode::Char('4') => {
                            current_filters.only_sshd = !current_filters.only_sshd;
                            msg = format!("Show Only SSH (sshd): {}", if current_filters.only_sshd { "ON".green().bold() } else { "OFF".red().bold() });
                        }
                        KeyCode::Char('h') | KeyCode::Char('?') => {
                            print_interactive_help();
                            changed = false;
                        }
                        _ => {
                            changed = false;
                        }
                    }

                    if changed {
                        let _ = filters_tx.send(current_filters.clone());
                        print_status_update(&msg);
                    }
                }
            }
        }
    });

    // Handle standard Ctrl+C signals (fallback)
    let handler_exit = exit_handler.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        handler_exit();
    });

    // 7. Start watching files and merge streams
    let (lines_tx, mut lines_rx) = tokio::sync::mpsc::channel::<(std::path::PathBuf, String)>(100);

    for file_path in args.files.clone() {
        let watcher = FileWatcher::new(file_path.clone(), args.tail);
        let mut file_rx = watcher.start().await?;
        let lines_tx = lines_tx.clone();
        tokio::spawn(async move {
            while let Some(line) = file_rx.recv().await {
                if lines_tx.send((file_path.clone(), line)).await.is_err() {
                    break;
                }
            }
        });
    }
    // Drop our copy of the sender to ensure channel closes when all tasks drop
    drop(lines_tx);

    // 8. Event loop
    while let Some((file_path, line)) = lines_rx.recv().await {
        // Apply CLI filters first
        if !parser_manager.filter_line(&line) {
            continue;
        }

        // Parse line
        let parsed_tuple = parser_manager.parse_line(&line);
        let parsed = parsed_tuple.as_ref().map(|(p, _)| p);

        // Apply interactive filters
        let active_filters = filters_rx.borrow().clone();
        if !apply_interactive_filters(&line, parsed, &active_filters) {
            continue;
        }

        // Geolocation lookup
        let mut geo_result = None;
        if !args.no_geo {
            let ip = parsed.and_then(|p| p.ip).or_else(|| formatter.extract_ip_fallback(&line));
            
            if let Some(ip_addr) = ip {
                stats.total_lookups.fetch_add(1, Ordering::Relaxed);
                
                if let Some(cached_val) = cache.get(ip_addr) {
                    stats.cache_hits.fetch_add(1, Ordering::Relaxed);
                    geo_result = cached_val;
                } else {
                    let lookup_val = geoip.lookup(ip_addr);
                    cache.insert(ip_addr, lookup_val.clone());
                    geo_result = lookup_val;
                }
            }
        }

        // Format and print
        let output = formatter.format_line(parsed, &line, geo_result.as_ref());
        if args.files.len() > 1 {
            let file_name = file_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            let colored_name = colorize_filename(file_name);
            println_raw!("[{}] {}", colored_name, output);
        } else {
            println_raw!("{}", output);
        }
    }

    // Restore terminal before exiting main
    crossterm::terminal::disable_raw_mode().ok();
    Ok(())
}

fn print_banner(args: &cli::Args, geoip: &GeoIp) {
    let title = "🥤 Cola v0.1.0 — Enhanced Log Viewer".cyan().bold();
    let format_label = "Format:    ".green().bold();
    let geoip_label = "GeoIP:     ".green().bold();
    let filters_label = "Filters:   ".green().bold();

    let format_str = match args.format {
        cli::LogFormat::Auto => "Auto-detecting...",
        cli::LogFormat::Nginx => "Nginx Combined Log Format",
        cli::LogFormat::Apache => "Apache Log Format",
        cli::LogFormat::Syslog => "Syslog / SSH Format",
    };

    println_raw!("{}", "┌────────────────────────────────────────────────────────┐".cyan());
    println_raw!("│  {}                  │", title);
    println_raw!("│                                                        │");
    
    for (i, file) in args.files.iter().enumerate() {
        let path_str = file.display().to_string();
        let display_str = if path_str.len() > 40 {
            format!("...{}", &path_str[path_str.len() - 37..])
        } else {
            path_str
        };
        if i == 0 {
            println_raw!("│  {} {:<40} │", "Monitoring:".green().bold(), display_str);
        } else {
            println_raw!("│              {:<40} │", display_str);
        }
    }
    
    println_raw!("│  {} {:<40} │", format_label, format_str);
    
    if args.no_geo {
        println_raw!("│  {} {:<40} │", geoip_label, "Disabled via --no-geo".red());
    } else {
        let status = if geoip.lookup(std::net::IpAddr::V4(std::net::Ipv4Addr::new(8, 8, 8, 8))).is_some() {
            "Active (GeoLite2-Country)".green().to_string()
        } else {
            "Active (Mock / DB Error fallback)".yellow().to_string()
        };
        println_raw!("│  {} {:<50} │", geoip_label, status);
    }

    let mut filter_desc = String::new();
    if let Some(ref inc) = args.include {
        filter_desc.push_str(&format!("Include: \"{}\" ", inc));
    }
    if let Some(ref exc) = args.exclude {
        filter_desc.push_str(&format!("Exclude: \"{}\" ", exc));
    }
    if filter_desc.is_empty() {
        filter_desc = "None".to_string();
    }
    println_raw!("│  {} {:<40} │", filters_label, filter_desc);
    println_raw!("{}", "└────────────────────────────────────────────────────────┘".cyan());
}

fn print_status_update(msg: &str) {
    println_raw!("\n🥤 [Cola] Toggled: {} [Press 'h' for help]\n", msg);
}

fn print_interactive_help() {
    println_raw!("\n┌──────────────────────────────────────────────────┐");
    println_raw!("│ 💡 Interactive Filter Controls                   │");
    println_raw!("│                                                  │");
    println_raw!("│  [1] Toggle: Hide Static Assets (.jpg, .css, etc)│");
    println_raw!("│  [2] Toggle: Show Only Errors (>= 400 / FAIL)    │");
    println_raw!("│  [3] Toggle: Hide Bots / Crawlers                │");
    println_raw!("│  [4] Toggle: Show Only SSH / Syslog logs         │");
    println_raw!("│  [h] Show this help menu                         │");
    println_raw!("│  [Ctrl+C] Exit and show cache stats              │");
    println_raw!("└──────────────────────────────────────────────────┘\n");
}

fn apply_interactive_filters(
    line: &str,
    parsed: Option<&formats::ParsedLine>,
    filters: &InteractiveFilters,
) -> bool {
    // 1. Hide static assets
    if filters.hide_static {
        let lower = line.to_lowercase();
        let is_static = lower.contains(".jpg")
            || lower.contains(".jpeg")
            || lower.contains(".png")
            || lower.contains(".gif")
            || lower.contains(".css")
            || lower.contains(".js")
            || lower.contains(".woff")
            || lower.contains(".ico");
        if is_static {
            return false;
        }
    }

    // 2. Only errors
    if filters.only_errors {
        let is_error = if let Some(p) = parsed {
            if p.service.as_deref() == Some("nginx") || p.service.as_deref() == Some("apache") {
                if let Some(ref status) = p.status {
                    status.parse::<u16>().map(|code| code >= 400).unwrap_or(false)
                } else {
                    false
                }
            } else if p.service.as_deref() == Some("nginx_error") || p.service.as_deref() == Some("apache_error") {
                true
            } else {
                p.method.as_deref() == Some("FAILED") || p.method.as_deref() == Some("INVALID")
            }
        } else {
            let lower = line.to_lowercase();
            lower.contains("failed") || lower.contains("error") || lower.contains("invalid")
        };
        if !is_error {
            return false;
        }
    }

    // 3. Hide bots
    if filters.hide_bots {
        let lower = line.to_lowercase();
        if lower.contains("bot") || lower.contains("spider") || lower.contains("crawler") {
            return false;
        }
    }

    // 4. Only sshd / syslog
    if filters.only_sshd {
        let is_sshd = if let Some(p) = parsed {
            p.service.as_deref() == Some("sshd")
        } else {
            line.contains("sshd")
        };
        if !is_sshd {
            return false;
        }
    }

    true
}

fn colorize_filename(name: &str) -> colored::ColoredString {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    name.hash(&mut hasher);
    let hash_val = hasher.finish();
    let colors = [
        colored::Color::Red,
        colored::Color::Green,
        colored::Color::Yellow,
        colored::Color::Blue,
        colored::Color::Magenta,
        colored::Color::Cyan,
        colored::Color::BrightRed,
        colored::Color::BrightGreen,
        colored::Color::BrightYellow,
        colored::Color::BrightBlue,
        colored::Color::BrightMagenta,
        colored::Color::BrightCyan,
    ];
    let color = colors[(hash_val % colors.len() as u64) as usize];
    name.color(color).bold()
}
