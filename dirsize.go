package main

import (
	"bufio"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"sync"
)

func scanDirectoryCLI() {
	// 获取用户输入目录路径
	reader := bufio.NewReader(os.Stdin)
	fmt.Print("Enter directory path: ")
	path, _ := reader.ReadString('\n')
	path = strings.TrimSpace(path)

	// 输入验证
	if path == "" {
		fmt.Fprintln(os.Stderr, "Empty path.")
		os.Exit(1)
	}

	// 检查路径有效性
	fileInfo, err := os.Stat(path)
	if os.IsNotExist(err) {
		fmt.Fprintf(os.Stderr, "Directory does not exist: %s\n", path)
		os.Exit(1)
	}
	if !fileInfo.IsDir() {
		fmt.Fprintf(os.Stderr, "Not a directory: %s\n", path)
		os.Exit(1)
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
					if parentDir == currentDir {
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
			return nil
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
				return nil
			}
			size := info.Size()
			fileSizes[currentPath] = size
			workChan <- FileTask{path: currentPath, size: size}
		}
		return nil
	})

	close(workChan)
	wg.Wait()

	// 结果整理与排序
	type Item struct {
		Path  string
		Size  int64
		IsDir bool
	}
	var items []Item

	// 收集直接子目录信息
	for dir, size := range dirSizes {
		if dir == rootDir {
			continue
		}
		if filepath.Dir(dir) == rootDir {
			relPath, _ := filepath.Rel(rootDir, dir)
			items = append(items, Item{
				Path:  relPath,
				Size:  size,
				IsDir: true,
			})
		}
	}

	// 收集直接子文件信息
	for file, size := range fileSizes {
		if filepath.Dir(file) == rootDir {
			relPath, _ := filepath.Rel(rootDir, file)
			items = append(items, Item{
				Path:  relPath,
				Size:  size,
				IsDir: false,
			})
		}
	}

	// 按大小降序排序
	sort.Slice(items, func(i, j int) bool {
		return items[i].Size > items[j].Size
	})

	// 格式化输出结果
	for _, item := range items {
		suffix := " (file)"
		if item.IsDir {
			suffix = " (dir)"
		}
		fmt.Printf("%10s %s%s\n", formatSize(item.Size), item.Path, suffix)
	}
}
