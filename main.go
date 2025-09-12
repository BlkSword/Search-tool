package main

import (
	"fmt"
	"io/fs"
	"net/http"
	"os"
	"path/filepath"
	"sort"
	"sync"
	"time"

	"github.com/gin-gonic/gin"
)

// 判断换算
func formatSize(bytes int64) string {
	if bytes < 1024 {
		return fmt.Sprintf("%d B", bytes)
	}
	kb := float64(bytes) / 1024
	if kb < 1024 {
		return fmt.Sprintf("%.1f KB", kb)
	}
	mb := kb / 1024
	if mb < 1024 {
		return fmt.Sprintf("%.1f MB", mb)
	}
	gb := mb / 1024
	return fmt.Sprintf("%.1f GB", gb)
}

type FileTask struct {
	path string
	size int64
}

type Item struct {
	Path          string `json:"path"`
	Size          int64  `json:"size"`
	SizeFormatted string `json:"sizeFormatted"`
	IsDir         bool   `json:"isDir"`
}

type ScanResult struct {
	Items              []Item  `json:"items"`
	TotalSize          int64   `json:"totalSize"`
	TotalSizeFormatted string  `json:"totalSizeFormatted"`
	ScanTime           float64 `json:"scanTime"`
	Path               string  `json:"path"`
}

// 历史记录项
type HistoryItem struct {
	Path       string    `json:"path"`
	ScanTime   time.Time `json:"scanTime"`
	TotalSize  int64     `json:"totalSize"`
	SizeFormat string    `json:"sizeFormat"`
	Items      []Item    `json:"items"` // 添加items字段来存储扫描结果
}

// 历史记录存储
var (
	history      []HistoryItem
	historyMutex sync.RWMutex
)

func init() {
	history = make([]HistoryItem, 0)
}

func scanDirectory(path string) (*ScanResult, error) {
	startTime := time.Now()

	// 输入验证
	if path == "" {
		return nil, fmt.Errorf("路径不能为空")
	}

	// 检查路径有效性
	fileInfo, err := os.Stat(path)
	if os.IsNotExist(err) {
		return nil, fmt.Errorf("目录不存在: %s", path)
	}
	if !fileInfo.IsDir() {
		return nil, fmt.Errorf("不是目录: %s", path)
	}

	// 数据结构初始化
	dirSizes := make(map[string]int64)    // 存储各目录累计大小
	fileSizes := make(map[string]int64)   // 存储文件单独大小
	rootDir := path                       // 根目录路径
	const numWorkers = 4                  // 并发工作协程数
	workChan := make(chan FileTask, 1024) // 任务队列
	var wg sync.WaitGroup                 // 协程同步组
	var dirMutex sync.Mutex               // 目录map的互斥锁

	// 启动工作协程池
	for i := 0; i < numWorkers; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for task := range workChan {
				dirPath := filepath.Dir(task.path)
				dirMutex.Lock()
				currentDir := dirPath
				// 向上逐级累加目录大小
				for {
					dirSizes[currentDir] += task.size
					if currentDir == rootDir {
						break
					}
					parentDir := filepath.Dir(currentDir)
					if parentDir == currentDir { // 防止无限循环
						break
					}
					currentDir = parentDir
				}
				dirMutex.Unlock()
			}
		}()
	}

	// 遍历目录树
	err = filepath.WalkDir(rootDir, func(currentPath string, d fs.DirEntry, err error) error {
		if err != nil {
			return nil // 跳过无法访问的文件
		}

		if d.IsDir() {
			// 初始化目录大小记录
			dirMutex.Lock()
			if _, ok := dirSizes[currentPath]; !ok {
				dirSizes[currentPath] = 0
			}
			dirMutex.Unlock()
		} else {
			// 处理文件大小统计
			info, err := d.Info()
			if err != nil {
				return nil // 跳过无法读取的文件
			}
			size := info.Size()
			fileSizes[currentPath] = size
			workChan <- FileTask{path: currentPath, size: size}
		}
		return nil
	})

	if err != nil {
		return nil, err
	}

	close(workChan)
	wg.Wait()

	// 结果整理与排序
	var items []Item
	var totalSize int64

	// 收集直接子目录信息
	for dir, size := range dirSizes {
		if dir == rootDir {
			continue // 跳过根目录自身
		}
		if filepath.Dir(dir) == rootDir {
			relPath, _ := filepath.Rel(rootDir, dir)
			items = append(items, Item{
				Path:          relPath,
				Size:          size,
				SizeFormatted: formatSize(size),
				IsDir:         true,
			})
			totalSize += size
		}
	}

	// 收集直接子文件信息
	for file, size := range fileSizes {
		if filepath.Dir(file) == rootDir {
			relPath, _ := filepath.Rel(rootDir, file)
			items = append(items, Item{
				Path:          relPath,
				Size:          size,
				SizeFormatted: formatSize(size),
				IsDir:         false,
			})
			totalSize += size
		}
	}

	// 按大小降序排序
	sort.Slice(items, func(i, j int) bool {
		return items[i].Size > items[j].Size
	})

	scanTime := time.Since(startTime).Seconds()

	// 添加到历史记录
	historyMutex.Lock()
	history = append(history, HistoryItem{
		Path:       path,
		ScanTime:   time.Now(),
		TotalSize:  totalSize,
		SizeFormat: formatSize(totalSize),
		Items:      items, // 保存items到历史记录
	})

	// 保持历史记录在合理范围内（最多保存50条）
	if len(history) > 50 {
		history = history[1:]
	}
	historyMutex.Unlock()

	return &ScanResult{
		Items:              items,
		TotalSize:          totalSize,
		TotalSizeFormatted: formatSize(totalSize),
		ScanTime:           scanTime,
		Path:               path,
	}, nil
}

func main() {
	r := gin.Default()

	// 加载HTML模板
	r.LoadHTMLGlob("templates/*")

	// 静态文件服务
	r.Static("/static", "./static")

	// 主页路由
	r.GET("/", func(c *gin.Context) {
		c.HTML(http.StatusOK, "index.html", gin.H{
			"title": "目录大小检索工具",
		})
	})

	// 扫描路由
	r.POST("/api/scan", func(c *gin.Context) {
		var req struct {
			Path string `json:"path" binding:"required"`
		}

		if err := c.ShouldBindJSON(&req); err != nil {
			c.JSON(http.StatusBadRequest, gin.H{"error": "请提供有效的目录路径"})
			return
		}

		result, err := scanDirectory(req.Path)
		if err != nil {
			c.JSON(http.StatusBadRequest, gin.H{"error": err.Error()})
			return
		}

		c.JSON(http.StatusOK, result)
	})

	// 获取历史记录路由
	r.GET("/api/history", func(c *gin.Context) {
		historyMutex.RLock()
		defer historyMutex.RUnlock()

		// 返回历史记录的副本，按时间倒序排列（最新的在前）
		historyCopy := make([]HistoryItem, len(history))
		for i, item := range history {
			historyCopy[len(history)-1-i] = item
		}

		c.JSON(http.StatusOK, historyCopy)
	})

	// 获取历史记录详情路由
	r.POST("/api/history-item", func(c *gin.Context) {
		var req struct {
			Path string `json:"path" binding:"required"`
		}

		if err := c.ShouldBindJSON(&req); err != nil {
			c.JSON(http.StatusBadRequest, gin.H{"error": "请提供有效的目录路径"})
			return
		}

		historyMutex.RLock()
		defer historyMutex.RUnlock()

		// 查找匹配的历史记录
		for i := len(history) - 1; i >= 0; i-- {
			if history[i].Path == req.Path {
				// 构造扫描结果
				result := &ScanResult{
					Items:              history[i].Items,
					TotalSize:          history[i].TotalSize,
					TotalSizeFormatted: history[i].SizeFormat,
					ScanTime:           0, // 历史记录没有扫描时间
					Path:               history[i].Path,
				}
				c.JSON(http.StatusOK, result)
				return
			}
		}

		c.JSON(http.StatusNotFound, gin.H{"error": "未找到该历史记录"})
	})

	// 启动服务器
	fmt.Println("服务器启动在 http://localhost:8080")
	r.Run(":8080")
}
