use search_tool::scan::{scan_directory, format_size};
use std::io::{self, Write};

#[tokio::main]
async fn main() {
    // 获取用户输入目录路径
    print!("Enter directory path: ");
    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .expect("Failed to read input");

    let path = input.trim();

    // 输入验证
    if path.is_empty() {
        eprintln!("Empty path.");
        std::process::exit(1);
    }

    // 扫描目录
    match scan_directory(path).await {
        Ok(result) => {
            // 格式化输出结果
            for item in &result.items {
                let suffix = if item.is_dir { " (dir)" } else { " (file)" };
                println!(
                    "{:10} {}{}",
                    format_size(item.size),
                    item.path,
                    suffix
                );
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
