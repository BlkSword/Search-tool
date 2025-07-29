@echo off
echo Launching the directory size retrieval tool...
echo.
echo Once the server is up, please visit: http://localhost:8080 in your browser
echo.
echo Press Ctrl C to stop the server
echo.
if exist searchtool.exe (
    searchtool.exe
) else (
    echo If no executable is found, compile it first: go build -o searchtool.exe main.go
    pause
    exit /b
)
pause