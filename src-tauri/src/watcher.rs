use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::sync::mpsc;
use tauri::{AppHandle, Emitter};

pub fn start_watcher(app: AppHandle) {
    let claude_dir = dirs::home_dir()
        .expect("Could not find home directory")
        .join(".claude");

    std::thread::spawn(move || {
        let (tx, rx) = mpsc::channel();

        let mut watcher = RecommendedWatcher::new(tx, Config::default())
            .expect("Failed to create file watcher");

        watcher
            .watch(&claude_dir, RecursiveMode::NonRecursive)
            .expect("Failed to watch .claude directory");

        loop {
            match rx.recv() {
                Ok(Ok(event)) => {
                    if !matches!(
                        event.kind,
                        EventKind::Modify(_) | EventKind::Create(_)
                    ) {
                        continue;
                    }

                    for path in &event.paths {
                        let filename = path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("");

                        match filename {
                            "stats-cache.json" => {
                                let _ = app.emit("stats-updated", ());
                            }
                            "history.jsonl" => {
                                let _ = app.emit("history-updated", ());
                            }
                            _ => {}
                        }
                    }
                }
                Ok(Err(e)) => {
                    eprintln!("Watcher error: {:?}", e);
                }
                Err(e) => {
                    eprintln!("Channel error: {:?}", e);
                    break;
                }
            }
        }

        // Keep watcher alive
        drop(watcher);
    });
}
