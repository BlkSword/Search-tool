use anyhow;
use dashmap::DashMap;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub path: String,
    pub name: String,
    pub size: i64,
    pub size_formatted: String,
    pub is_dir: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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

#[derive(Debug, Clone)]
pub struct CacheEntry {
    result: ScanResult,
    dir_mtime: chrono::DateTime<chrono::Local>,
}

pub struct ScanCache {
    cache: DashMap<String, CacheEntry>,
    max_entries: usize,
    max_size_bytes: usize,
    current_size: DashMap<String, usize>,
}

impl ScanCache {
    pub fn new(max_entries: usize, max_size_mb: usize) -> Self {
        ScanCache {
            cache: DashMap::new(),
            max_entries,
            max_size_bytes: max_size_mb * 1024 * 1024,
            current_size: DashMap::new(),
        }
    }

    pub fn get(&self, path: &str) -> Option<CacheEntry> {
        self.cache.get(path).map(|entry| entry.clone())
    }

    pub fn insert(&self, path: String, result: ScanResult) {
        // 估算当前条目大小
        let entry_size = self.estimate_size(&result);

        // 检查是否超过最大条目数或总大小限制
        if self.cache.len() >= self.max_entries
            || self.get_total_size() + entry_size > self.max_size_bytes
        {
            self.evict_oldest();
        }

        self.current_size.insert(path.clone(), entry_size);
        self.cache.insert(
            path,
            CacheEntry {
                result,
                dir_mtime: chrono::Local::now(),
            },
        );
    }

    fn estimate_size(&self, result: &ScanResult) -> usize {
        // 估算每个条目占用的内存（粗略估计）
        result.items.len() * (100 + std::mem::size_of::<Item>())
    }

    fn get_total_size(&self) -> usize {
        self.current_size.iter().map(|entry| *entry.value()).sum()
    }

    fn evict_oldest(&self) {
        let mut entries: Vec<_> = self.cache.iter().collect();
        entries.sort_by(|a, b| a.value().dir_mtime.cmp(&b.value().dir_mtime));
        if let Some(entry) = entries.first() {
            let key = entry.key().clone();
            self.current_size.remove(&key);
            self.cache.remove(&key);
        }
    }

    pub fn invalidate(&self, path: &str) {
        let keys_to_remove: Vec<String> = self
            .cache
            .iter()
            .filter(|entry| entry.key().starts_with(path))
            .map(|entry| entry.key().clone())
            .collect();
        for key in keys_to_remove {
            self.current_size.remove(&key);
            self.cache.remove(&key);
        }
    }

    #[allow(dead_code)]
    pub fn clear(&self) {
        self.cache.clear();
        self.current_size.clear();
    }
}

lazy_static::lazy_static! {
    // 减少缓存条目数量，限制总内存使用为 100MB
    static ref SCAN_CACHE: ScanCache = ScanCache::new(50, 100);
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

pub async fn scan_directory(path: &str, force_refresh: bool) -> Result<ScanResult, anyhow::Error> {
    let start_time = std::time::Instant::now();

    if path.trim().is_empty() {
        return Err(anyhow::anyhow!("路径不能为空"));
    }

    let path_buf = PathBuf::from(path);

    let metadata = match fs::metadata(&path_buf).await {
        Ok(m) => m,
        Err(e) => {
            return Err(anyhow::anyhow!("无法访问路径: {}", e));
        }
    };

    if !metadata.is_dir() {
        return Err(anyhow::anyhow!("不是目录"));
    }

    let canonical_path = match fs::canonicalize(&path_buf).await {
        Ok(p) => p,
        Err(e) => {
            return Err(anyhow::anyhow!("路径规范化失败: {}", e));
        }
    };

    let root_dir = canonical_path.to_string_lossy().replace('\\', "/");

    let mtime = match metadata.modified() {
        Ok(m) => m,
        Err(_) => std::time::SystemTime::UNIX_EPOCH,
    };
    let mtime_datetime: chrono::DateTime<chrono::Local> = mtime.into();

    if !force_refresh {
        if let Some(cached) = SCAN_CACHE.get(&root_dir) {
            if cached.dir_mtime >= mtime_datetime {
                let mut result = cached.result.clone();
                result.scan_time = 0.0;
                return Ok(result);
            }
        }
    }

    SCAN_CACHE.invalidate(&root_dir);

    let root_dir_for_processing = root_dir.clone();

    let (dir_sizes, file_sizes) = tokio::task::spawn_blocking(move || {
        scan_directory_blocking(&canonical_path, &root_dir_for_processing)
    })
    .await??;

    // 预分配容量以减少重新分配
    let mut items = Vec::with_capacity(dir_sizes.len() + file_sizes.len());
    let mut total_size = 0i64;

    for (dir, size) in dir_sizes.iter() {
        if dir == &root_dir {
            continue;
        }

        if let Ok(rel_path) = Path::new(dir).strip_prefix(&root_dir) {
            let rel_path_str = rel_path.to_string_lossy().to_string();
            if !rel_path_str.is_empty() {
                let name = Path::new(&rel_path_str)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(&rel_path_str)
                    .to_string();
                items.push(Item {
                    path: rel_path_str,
                    name,
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
                let name = Path::new(&rel_path_str)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(&rel_path_str)
                    .to_string();
                items.push(Item {
                    path: rel_path_str,
                    name,
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

    let result = ScanResult {
        items,
        total_size,
        total_size_formatted: format_size(total_size),
        scan_time,
        path: path.to_string(),
    };

    SCAN_CACHE.insert(root_dir.clone(), result.clone());

    Ok(result)
}

fn scan_directory_blocking(
    path: &Path,
    root_dir: &str,
) -> Result<(HashMap<String, i64>, HashMap<String, i64>), anyhow::Error> {
    // 使用流式处理，避免一次性收集所有文件
    let dir_sizes = DashMap::new();
    let file_sizes = DashMap::new();
    let root_path = Path::new(root_dir).to_path_buf();

    // 分批处理文件以减少内存压力
    let batch_size = 10000;
    let mut batch: Vec<(PathBuf, i64)> = Vec::with_capacity(batch_size);

    // 使用优化的文件收集方法
    for entry in collect_files_optimized(path)? {
        let (file_path, size) = entry;

        // 添加到文件大小映射
        if let Some(path_str) = file_path.to_str() {
            let normalized_path = path_str.replace('\\', "/");
            file_sizes.insert(normalized_path, size);
        }

        // 添加到批次
        batch.push((file_path, size));

        // 批次满了就处理
        if batch.len() >= batch_size {
            process_batch(&batch, &dir_sizes, &root_path);
            batch.clear();
        }
    }

    // 处理剩余的文件
    if !batch.is_empty() {
        process_batch(&batch, &dir_sizes, &root_path);
    }

    // 转换为普通 HashMap
    let mut dir_sizes_map = HashMap::with_capacity(dir_sizes.len());
    for (key, value) in dir_sizes.into_iter() {
        dir_sizes_map.insert(key, value);
    }

    let mut file_sizes_map = HashMap::with_capacity(file_sizes.len());
    for (key, value) in file_sizes.into_iter() {
        file_sizes_map.insert(key, value);
    }

    Ok((dir_sizes_map, file_sizes_map))
}

fn process_batch(batch: &[(PathBuf, i64)], dir_sizes: &DashMap<String, i64>, root_path: &Path) {
    batch.par_iter().for_each(|(file_path, size)| {
        if let Some(parent) = file_path.parent() {
            for ancestor in parent.ancestors() {
                if ancestor == root_path || ancestor == Path::new("") {
                    break;
                }
                if let Some(dir_path) = ancestor.to_str() {
                    let mut sizes = dir_sizes.entry(dir_path.to_string()).or_default();
                    *sizes += size;
                }
            }
        }
    });
}

// 备用方案：使用更高效的文件收集方法
fn collect_files_optimized(path: &Path) -> Result<Vec<(PathBuf, i64)>, anyhow::Error> {
    let mut files = Vec::new();
    let mut stack = vec![path.to_path_buf()];

    while let Some(current_path) = stack.pop() {
        if let Ok(entries) = std::fs::read_dir(&current_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Ok(metadata) = path.metadata() {
                    if metadata.is_dir() {
                        stack.push(path);
                    } else if metadata.is_file() {
                        files.push((path, metadata.len() as i64));
                    }
                }
            }
        }
    }

    Ok(files)
}
