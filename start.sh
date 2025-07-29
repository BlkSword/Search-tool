#!/bin/bash

echo "正在启动目录大小检索工具..."
echo
echo "服务器启动后，请在浏览器中访问: http://localhost:8080"
echo
echo "按 Ctrl+C 停止服务器"
echo

if [ -f "searchtool" ]; then
    ./searchtool
else
    echo "未找到可执行文件，请先编译: go build -o searchtool main.go"
    exit 1
fi