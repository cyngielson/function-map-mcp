// function-map-mcp - https://github.com/cyngielson/function-map-mcp
// Copyright (c) 2025-2026 cyngielson. MIT License. Free to use, attribution appreciated.
//! File Watcher - Real-time file change monitoring
//! 
//! Uproszczony system obserwacji zmian:
//! - Asynchroniczne śledzenie zmian plików
//! - Debouncing dla wydajności  
//! - Filtrowanie niepotrzebnych zdarzeń
//! - Cross-platform compatibility

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher, Config};
use tokio::sync::{mpsc, RwLock};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use anyhow::{Result, Context};
use log::{info, debug, warn};
use serde::{Deserialize, Serialize};
use chrono::Utc;
/// File change event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChangeEvent {
    pub file_path: PathBuf,
    pub change_type: ChangeType,
    pub timestamp: u64,
}

/// Types of file changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChangeType {
    Created,
    Modified,
    Deleted,
    Renamed { from: PathBuf, to: PathBuf },
}

/// File watcher configuration
#[derive(Debug, Clone)]
pub struct FileWatcherConfig {
    pub debounce_duration: Duration,
    pub watched_extensions: HashSet<String>,
    pub ignored_patterns: Vec<String>,
    pub recursive: bool,
    pub buffer_size: usize,
}

impl Default for FileWatcherConfig {
    fn default() -> Self {
        let mut watched_extensions = HashSet::new();
        watched_extensions.extend([
            "rs", "py", "js", "ts", "tsx", "jsx", "go", "java", "c", "cpp", "h", "hpp"
        ].iter().map(|s| s.to_string()));

        Self {
            debounce_duration: Duration::from_millis(500),
            watched_extensions,
            ignored_patterns: vec![
                "target/".to_string(),
                "node_modules/".to_string(),
                ".git/".to_string(),
                "__pycache__/".to_string(),
                "*.pyc".to_string(),
                ".DS_Store".to_string(),
                "Thumbs.db".to_string(),
            ],
            recursive: true,
            buffer_size: 1000,
        }
    }
}

/// File watcher statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatcherStats {
    pub files_watched: usize,
    pub events_processed: usize,
    pub events_filtered: usize,
    pub last_event_time: Option<u64>,
    pub watching_since: u64,
}

/// File change callback type - simplified for MCP use
pub type FileChangeCallback = fn(&str, &FileChangeEvent) -> ();

/// Real-time file watcher
pub struct FileWatcher {
    config: FileWatcherConfig,
    watcher: Option<RecommendedWatcher>,
    event_sender: Option<mpsc::UnboundedSender<FileChangeEvent>>,
    debounce_map: RwLock<HashMap<PathBuf, Instant>>,
    stats: RwLock<WatcherStats>,
    callback: Option<FileChangeCallback>,
}

impl FileWatcher {
    /// Create new file watcher
    pub fn new(config: FileWatcherConfig) -> Self {
        let stats = WatcherStats {
            files_watched: 0,
            events_processed: 0,
            events_filtered: 0,
            last_event_time: None,
            watching_since: chrono::Utc::now().timestamp() as u64,
        };

        Self {
            config,
            watcher: None,
            event_sender: None,
            debounce_map: RwLock::new(HashMap::new()),
            stats: RwLock::new(stats),
            callback: None,
        }
    }

    /// Start watching a directory
    pub async fn start_watching<P: AsRef<Path>>(&mut self, path: P, callback: FileChangeCallback) -> Result<()> {
        let path = path.as_ref();
        info!("📁 Starting file watcher for: {:?}", path);

        self.callback = Some(callback);

        // Create event channel
        let (tx, mut rx) = mpsc::unbounded_channel::<FileChangeEvent>();
        self.event_sender = Some(tx.clone());

        // Create filesystem watcher
        let mut watcher = RecommendedWatcher::new(
            move |res: notify::Result<Event>| {
                match res {
                    Ok(event) => {
                        if let Some(file_event) = Self::convert_notify_event(event) {
                            if let Err(e) = tx.send(file_event) {
                                warn!("Failed to send file event: {}", e);
                            }
                        }
                    }
                    Err(e) => warn!("File watcher error: {}", e),
                }
            },
            Config::default(),
        )?;

        // Start watching
        let mode = if self.config.recursive {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        };

        watcher.watch(path, mode)
            .with_context(|| format!("Failed to watch path: {:?}", path))?;

        self.watcher = Some(watcher);

        // Count files being watched
        let file_count = self.count_watched_files(path).await;
        self.stats.write().await.files_watched = file_count;

        info!("👁️ Watching {} files in {:?}", file_count, path);

        // Start event processing task
        let config = self.config.clone();
        let debounce_map = std::sync::Arc::new(tokio::sync::RwLock::new(HashMap::new()));
        let stats = std::sync::Arc::new(tokio::sync::RwLock::new(WatcherStats {
            files_watched: file_count,
            events_processed: 0,
            events_filtered: 0,
            last_event_time: None,
            watching_since: chrono::Utc::now().timestamp() as u64,
        }));
        let callback_fn = callback;

        tokio::spawn(async move {
            let mut batch_events = Vec::new();
            let mut last_batch_time = Instant::now();
            
            while let Some(event) = rx.recv().await {
                // Apply debouncing
                if Self::should_debounce_async(&event, &debounce_map, &config).await {
                    let mut stats = stats.write().await;
                    stats.events_filtered += 1;
                    continue;
                }

                // Filter unwanted events
                if !Self::should_process_event(&event, &config) {
                    let mut stats = stats.write().await;
                    stats.events_filtered += 1;
                    continue;
                }

                batch_events.push(event);

                // Process batch if timeout reached or batch is full
                let should_process_batch = 
                    batch_events.len() >= 10 ||
                    last_batch_time.elapsed() > Duration::from_millis(200);

                if should_process_batch && !batch_events.is_empty() {
                    debug!("📦 Processing batch of {} file events", batch_events.len());
                    
                    for event in &batch_events {
                        callback_fn("repo_id", event);
                    }

                    // Update stats
                    {
                        let mut stats = stats.write().await;
                        stats.events_processed += batch_events.len();
                        stats.last_event_time = Some(chrono::Utc::now().timestamp() as u64);
                    }

                    batch_events.clear();
                    last_batch_time = Instant::now();
                }
            }
        });

        Ok(())
    }

    /// Stop watching
    pub async fn stop_watching(&mut self) -> Result<()> {
        if let Some(watcher) = self.watcher.take() {
            drop(watcher);
            info!("⏹️ File watcher stopped");
        }
        
        if let Some(sender) = self.event_sender.take() {
            drop(sender);
        }

        Ok(())
    }

    /// Get watcher statistics
    pub async fn get_stats(&self) -> WatcherStats {
        self.stats.read().await.clone()
    }

    /// Convert notify event to our event type
    fn convert_notify_event(event: Event) -> Option<FileChangeEvent> {
        let timestamp = chrono::Utc::now().timestamp() as u64;

        match event.kind {
            EventKind::Create(_) => {
                if let Some(path) = event.paths.first() {
                    Some(FileChangeEvent {
                        file_path: path.clone(),
                        change_type: ChangeType::Created,
                        timestamp,
                    })
                } else {
                    None
                }
            }
            EventKind::Modify(_) => {
                if let Some(path) = event.paths.first() {
                    Some(FileChangeEvent {
                        file_path: path.clone(),
                        change_type: ChangeType::Modified,
                        timestamp,
                    })
                } else {
                    None
                }
            }
            EventKind::Remove(_) => {
                if let Some(path) = event.paths.first() {
                    Some(FileChangeEvent {
                        file_path: path.clone(),
                        change_type: ChangeType::Deleted,
                        timestamp,
                    })
                } else {
                    None
                }
            }
            _ => None, // Ignore other event types
        }
    }

    /// Check if event should be debounced (async version)
    async fn should_debounce_async(
        event: &FileChangeEvent,
        debounce_map: &std::sync::Arc<tokio::sync::RwLock<HashMap<PathBuf, Instant>>>,
        config: &FileWatcherConfig,
    ) -> bool {
        let now = Instant::now();
        let mut map = debounce_map.write().await;
        
        if let Some(&last_time) = map.get(&event.file_path) {
            if now.duration_since(last_time) < config.debounce_duration {
                // Update timestamp but don't process
                map.insert(event.file_path.clone(), now);
                return true;
            }
        }
        
        map.insert(event.file_path.clone(), now);
        false
    }

    /// Check if event should be processed
    fn should_process_event(event: &FileChangeEvent, config: &FileWatcherConfig) -> bool {
        let path_str = event.file_path.to_string_lossy();
        
        // Check ignored patterns
        for pattern in &config.ignored_patterns {
            if pattern.contains('*') {
                // Simple glob matching
                let pattern_regex = pattern.replace('*', ".*");
                if regex::Regex::new(&pattern_regex)
                    .map(|r| r.is_match(&path_str))
                    .unwrap_or(false)
                {
                    return false;
                }
            } else if path_str.contains(pattern) {
                return false;
            }
        }

        // Check file extension
        if let Some(extension) = event.file_path.extension().and_then(|s| s.to_str()) {
            config.watched_extensions.contains(extension)
        } else {
            false
        }
    }

    /// Count files that would be watched
    async fn count_watched_files<P: AsRef<Path>>(&self, path: P) -> usize {
        let mut count = 0;
        
        if let Ok(entries) = walkdir::WalkDir::new(path)
            .follow_links(false)
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
        {
            for entry in entries {
                if entry.file_type().is_file() {
                    if let Some(extension) = entry.path().extension().and_then(|s| s.to_str()) {
                        if self.config.watched_extensions.contains(extension) {
                            let path_str = entry.path().to_string_lossy();
                            let should_ignore = self.config.ignored_patterns
                                .iter()
                                .any(|pattern| path_str.contains(pattern));
                            
                            if !should_ignore {
                                count += 1;
                            }
                        }
                    }
                }
            }
        }
        
        count
    }
}