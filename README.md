# Search-tool

一个基于Go语言的目录大小检索工具，提供Web界面。

## 功能特点

- 🖥️ **命令行工具**: 快速扫描目录大小
- 🌐 **Web界面**: 现代化的可视化界面
- 📊 **详细统计**: 显示文件和文件夹大小分布
- ⚡ **高性能**: 多协程并发处理

## 使用方式

启动Web服务器：
双击searchtool.exe，启动服务端

然后在浏览器中访问：`http://localhost:8080`

## 技术栈

- **后端**: Go + Gin框架
- **前端**: HTML5 + Bootstrap 5 + JavaScript
- **并发**: Go协程池
- **样式**: 现代化渐变设计

## 开发说明

项目结构：
```
Search-tool/
├── main.go          # Web应用主程序
├── dirsize.go       # 命令行工具
├── templates/       # HTML模板
│   └── index.html   # 主页面
├── static/          # 静态资源
├── go.mod           # Go模块文件
└── README.md        # 项目说明
```
