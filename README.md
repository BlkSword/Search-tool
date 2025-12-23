# Directory

一个基于 Rust + Tauri 的桌面应用程序，用于可视化扫描和分析目录与文件的占用情况。

## 主要特点

- 便携版本：单个 EXE 文件，双击即可运行，无需安装
- 极速扫描：并行处理优化，1GB 文件仅需约 0.06 秒
- 智能缓存：基于目录修改时间自动更新缓存
- 可视化图表：使用 Chart.js 展示目录占用分布
- 目录树导航：支持文件夹层级浏览和前进返回功能
- 跨平台支持：Windows、macOS、Linux

## 解决的问题

当磁盘空间突然爆满时，快速定位占用大量空间的目录和文件。

## 使用说明

### 快速上手

1. 双击 `search-tool.exe` 启动应用
2. 点击浏览按钮选择要扫描的目录，或直接输入路径
3. 点击「扫描」按钮开始分析

### 开发模式运行

```bash
# 安装 Tauri CLI（如未安装）
cargo install tauri-cli

# 启动开发模式
cargo tauri dev
```

### 构建便携版

```bash
# Windows
.\build.bat

# Linux/macOS
chmod +x build.sh
./build.sh
```

构建完成后，可执行文件位于 `src-tauri/target/release/bundle/msi/` 目录（Windows）或相应平台的输出目录。

## 技术栈

- **后端**：Rust + Tauri + Rayon（并行计算） + DashMap（并发缓存）
- **前端**：HTML5 + Bootstrap 5 + JavaScript + Chart.js
- **架构**：桌面应用程序，通过 Tauri API 实现前后端通信

## 项目结构

```
Search-tool/
├── src-tauri/              # Rust 后端代码
│   ├── app/                # 前端资源
│   │   ├── index.html      # 主页面
│   │   └── src/            # 前端脚本和样式
│   │       ├── main.js     # 主逻辑脚本
│   │       └── style.css   # 样式文件
│   ├── src/                # Rust 源码
│   │   ├── main.rs         # Tauri 入口
│   │   ├── commands.rs     # 命令处理器
│   │   └── scan.rs         # 扫描核心逻辑
│   ├── Cargo.toml          # Rust 依赖配置
│   ├── tauri.conf.json     # Tauri 配置
│   └── icons/              # 应用图标
├── build.sh                # Linux/macOS 构建脚本
├── build.bat               # Windows 构建脚本
└── README.md               # 项目说明文档
```

## 功能特性

### 目录扫描

扫描指定目录，递归分析所有子目录和文件的大小，支持按名称、大小、类型排序显示。

### 智能缓存机制

首次扫描后，系统会自动缓存目录结构。当用户再次扫描同一目录时，系统会检查目录的修改时间：

- 若目录无变化：直接使用缓存数据，秒级响应
- 若目录有变化：增量更新缓存，确保数据准确

### 可视化图表

使用 Chart.js 绘制目录占用分布图，直观展示哪些子目录占用空间最大。

### 目录树导航

左侧显示可展开的目录树，支持：

- 点击文件夹展开子目录
- 点击文件显示详细信息
- 前进和返回按钮切换浏览历史

### 历史记录

自动保存最近 50 条扫描记录，方便快速访问常用目录。

## 开发指南

### 前端开发

前端代码位于 `src-tauri/app/` 目录，使用原生 JavaScript 和 Bootstrap 5。通过 Tauri 的 `invoke` API 与 Rust 后端通信：

```javascript
// 调用 Rust 扫描函数
const result = await invoke('scan_directory', { path: '/some/path' });
```

### 后端开发

后端代码位于 `src-tauri/src/` 目录：

- `main.rs`：Tauri 应用入口，负责注册命令处理器
- `commands.rs`：实现 Tauri 命令，处理前端请求并返回结果
- `scan.rs`：目录扫描核心逻辑，包含并行计算和缓存管理

### 添加新功能

1. 在 `src-tauri/src/scan.rs` 中实现核心逻辑
2. 在 `src-tauri/src/commands.rs` 中添加命令接口
3. 在 `src-tauri/src/main.rs` 中注册新命令
4. 在 `src-tauri/app/index.html` 和对应脚本中调用命令

## 性能优化

本项目采用以下优化策略：

- **并行计算**：使用 Rayon 库实现多线程并行扫描
- **并发缓存**：使用 DashMap 替代 Mutex，减少锁竞争
- **字符串优化**：减少不必要的路径字符串分配
- **增量更新**：基于修改时间检查，避免重复扫描

## 许可证

MIT License
