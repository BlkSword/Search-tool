<table>
<tr>
<td><img src="https://free.picui.cn/free/2025/07/29/6888e609ad2f0.png" alt="图片1" width="500"/></td>
</tr>
</table>

# Search-tool

一个基于Go语言的可视化检索目录/文件占用情况的工具

## 特点

一键启动，无需安装

可视化界面且检索极快(1G仅需0.06s)

## 背景

主要是为了解决，突然发现磁盘爆满。但是不知道是什么占用了的情况

## 使用方式

启动Web服务器：
双击searchtool.exe，启动服务端

然后在浏览器中访问：`http://localhost:8080`

## 技术栈

- **后端**: Go + Gin框架
- **前端**: HTML5 + Bootstrap 5 + JavaScript

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
