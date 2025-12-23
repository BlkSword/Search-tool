use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::sync::Mutex;
use std::sync::Arc;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub path: String,
    pub size: i64,
    pub size_formatted: String,
    pub is_dir: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub items: Vec<Item>,
    pub total_size: i64,
    pub total_size_formatted: String,
    pub scan_time: f64,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryItem {
    pub path: String,
    #[serde(with = "chrono::serde::ts_seconds")]
    pub scan_time: chrono::DateTime<chrono::Utc>,
    pub total_size: i64,
    pub size_format: String,
    pub items: Vec<Item>,
}

pub fn format_size(bytes: i64) -> String {
    if bytes < 1024 {
        return format!("{} B", bytes);
    }
    let kb = bytes as f64 / 1024.0;
    if kb < 1024.0 {
        return format!("{:.1} KB", kb);
    }
    let mb = kb / 1024.0;
    if mb < 1024.0 {
        return format!("{:.1} MB", mb);
    }
    let gb = mb / 1024.0;
    format!("{:.1} GB", gb)
}

pub async fn scan_directory(path: &str) -> Result<ScanResult, Box<dyn std::error::Error>> {
    let start_time = std::time::Instant::now();

    if path.is_empty() {
        return Err("路径不能为空".into());
    }

    let path_buf = PathBuf::from(path);
    let metadata = fs::metadata(&path_buf).await?;
    if !metadata.is_dir() {
        return Err("不是目录".into());
    }

    let canonical_path = fs::canonicalize(&path_buf).await?;
    let root_dir = canonical_path.to_string_lossy().to_string();

    let dir_sizes = Arc::new(Mutex::new(HashMap::new()));
    let file_sizes = Arc::new(Mutex::new(HashMap::new()));

    // 使用并发工作池模式
    let (tx, mut rx) = mpsc::channel::<(String, i64)>(1024);
    let dir_sizes_worker = Arc::clone(&dir_sizes);
    let root_dir_clone = root_dir.clone();

    // 启动工作协程处理任务队列
    let handle = tokio::spawn(async move {
        while let Some((file_path, size)) = rx.recv().await {
            let mut current_dir = Path::new(&file_path).parent();
            while let Some(dir) = current_dir {
                let dir_path = dir.to_string_lossy().to_string();
                if dir_path == root_dir_clone || dir_path.is_empty() {
                    // 添加到根目录
                    dir_sizes_worker.lock().await.entry(root_dir_clone.clone()).and_modify(|s| *s += size).or_insert(size);
                    break;
                }

                dir_sizes_worker
                    .lock()
                    .await
                    .entry(dir_path.clone())
                    .and_modify(|s| *s += size)
                    .or_insert(size);

                current_dir = dir.parent();
            }
        }
    });

    scan_recursive(&canonical_path, &root_dir, &tx).await?;
    drop(tx);
    handle.await?;

    let dir_sizes = dir_sizes.lock().await;
    let file_sizes = file_sizes.lock().await;

    let mut items = Vec::new();
    let mut total_size = 0i64;

    for (dir, size) in dir_sizes.iter() {
        if dir == &root_dir {
            continue;
        }

        if let Ok(rel_path) = Path::new(dir).strip_prefix(&root_dir) {
            let rel_path_str = rel_path.to_string_lossy().to_string();
            if !rel_path_str.is_empty() {
                items.push(Item {
                    path: rel_path_str,
                    size: *size,
                    size_formatted: format_size(*size),
                    is_dir: true,
                });
                total_size += size;
            }
        }
    }

    for (file, size) in file_sizes.iter() {
        if let Ok(rel_path) = Path::new(file).strip_prefix(&root_dir) {
            let rel_path_str = rel_path.to_string_lossy().to_string();
            if !rel_path_str.is_empty() {
                items.push(Item {
                    path: rel_path_str,
                    size: *size,
                    size_formatted: format_size(*size),
                    is_dir: false,
                });
                total_size += size;
            }
        }
    }

    items.sort_by(|a, b| b.size.cmp(&a.size));

    let scan_time = start_time.elapsed().as_secs_f64();

    Ok(ScanResult {
        items,
        total_size,
        total_size_formatted: format_size(total_size),
        scan_time,
        path: path.to_string(),
    })
}

async fn scan_recursive(
    path: &Path,
    root_dir: &str,
    tx: &mpsc::Sender<(String, i64)>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut entries = fs::read_dir(path).await?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        let metadata = entry.metadata().await?;

        if metadata.is_dir() {
            scan_recursive(&path, root_dir, tx).await?;
        } else {
            let size = metadata.len() as i64;
            let file_path = path.to_string_lossy().to_string();
            let _ = tx.send((file_path, size)).await;
        }
    }

    Ok(())
}
