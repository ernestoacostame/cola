use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::sync::mpsc;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};

pub struct FileWatcher {
    path: PathBuf,
    tail_lines: usize,
}

impl FileWatcher {
    pub fn new(path: PathBuf, tail_lines: usize) -> Self {
        Self { path, tail_lines }
    }

    /// Starts watching the file and returns a receiver stream of lines
    pub async fn start(self) -> anyhow::Result<mpsc::Receiver<String>> {
        let (tx, rx) = mpsc::channel(100);
        let path = self.path.clone();
        let tail_lines = self.tail_lines;

        tokio::spawn(async move {
            // 1. Wait for file to exist if it doesn't
            while !path.exists() {
                tokio::time::sleep(Duration::from_millis(500)).await;
            }

            // 2. Read initial tail lines and get initial offset
            let mut offset = 0;
            match read_last_n_lines(&path, tail_lines) {
                Ok((lines, init_offset)) => {
                    offset = init_offset;
                    for line in lines {
                        if tx.send(line).await.is_err() {
                            return;
                        }
                    }
                }
                Err(e) => {
                    eprintln!("⚠️  Error reading initial lines: {}", e);
                }
            }

            // 3. Set up notify watcher
            let (event_tx, mut event_rx) = mpsc::channel(10);
            let mut watcher = match RecommendedWatcher::new(
                move |res| {
                    if let Ok(event) = res {
                        let _ = event_tx.blocking_send(event);
                    }
                },
                Config::default(),
            ) {
                Ok(w) => w,
                Err(e) => {
                    eprintln!("⚠️  Failed to create file watcher: {}. Falling back to polling.", e);
                    // Create a dummy struct that won't watch anything
                    return;
                }
            };

            // Watch parent directory to handle file rotation/deletion/creation cleanly
            let watch_path = if let Some(parent) = path.parent() {
                if parent.as_os_str().is_empty() {
                    Path::new(".")
                } else {
                    parent
                }
            } else {
                Path::new(".")
            };

            if let Err(e) = watcher.watch(watch_path, RecursiveMode::NonRecursive) {
                eprintln!("⚠️  Failed to watch path: {}. Falling back to polling.", e);
            }

            // Keep the watcher in scope
            let _watcher = watcher;

            // 4. Main monitoring loop
            // We use a ticker as a fallback in case notify events miss something
            let mut ticker = tokio::time::interval(Duration::from_millis(250));
            
            loop {
                tokio::select! {
                    _ = event_rx.recv() => {
                        if let Err(e) = read_new_lines(&path, &mut offset, &tx).await {
                            eprintln!("⚠️  Read error: {}", e);
                        }
                    }
                    _ = ticker.tick() => {
                        if let Err(e) = read_new_lines(&path, &mut offset, &tx).await {
                            eprintln!("⚠️  Read error: {}", e);
                        }
                    }
                }
            }
        });

        Ok(rx)
    }
}

/// Read the last N lines of a file and return them alongside the final offset
fn read_last_n_lines(path: &Path, n: usize) -> std::io::Result<(Vec<String>, u64)> {
    let file = File::open(path)?;
    let metadata = file.metadata()?;
    let file_len = metadata.len();

    if n == 0 {
        return Ok((Vec::new(), file_len));
    }

    // Attempt to read only a chunk from the end to support massive files efficiently
    let chunk_size = std::cmp::max(65536, (n as u64) * 512); // 64KB or N * 512 bytes
    let seek_pos = if file_len > chunk_size {
        file_len - chunk_size
    } else {
        0
    };

    let mut file = File::open(path)?;
    file.seek(SeekFrom::Start(seek_pos))?;
    
    let reader = BufReader::new(file);
    let mut lines = std::collections::VecDeque::with_capacity(n);
    let mut is_first = true;

    for line_result in reader.lines() {
        let line = line_result?;
        // If we sought to a non-zero position, discard the first line since it is likely partial
        if seek_pos > 0 && is_first {
            is_first = false;
            continue;
        }
        is_first = false;
        
        if lines.len() == n {
            lines.pop_front();
        }
        lines.push_back(line);
    }

    Ok((lines.into_iter().collect(), file_len))
}

/// Reads any newly appended lines since `offset`, updating `offset` and sending to channel
async fn read_new_lines(
    path: &Path,
    offset: &mut u64,
    tx: &mpsc::Sender<String>,
) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let file = File::open(path)?;
    let metadata = file.metadata()?;
    let current_len = metadata.len();

    // Check for truncation / rotation
    if current_len < *offset {
        // Reset to beginning
        *offset = 0;
    }

    if current_len > *offset {
        let mut file = File::open(path)?;
        file.seek(SeekFrom::Start(*offset))?;
        let reader = BufReader::new(file);

        for line_result in reader.lines() {
            let line = line_result?;
            if tx.send(line).await.is_err() {
                break;
            }
        }

        *offset = current_len;
    }

    Ok(())
}
