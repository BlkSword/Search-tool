import './style.css'
import Chart from 'chart.js/auto'

let currentData = null;
let currentSort = { column: 'size', direction: 'desc' };
let currentScanPath = '';
let historyModal = null;
let typeSizeChart = null;
let topItemsChart = null;

let navigationHistory = [];
let navigationIndex = -1;
let rootScanData = null;

const invoke = window.__TAURI__?.invoke;

document.addEventListener('DOMContentLoaded', function () {
    historyModal = new bootstrap.Modal(document.getElementById('historyModal'));
    updateSortIndicators();
    loadHistory();
    updateNavigationButtons();
});

document.getElementById('backBtn').addEventListener('click', navigateBack);
document.getElementById('forwardBtn').addEventListener('click', navigateForward);
document.getElementById('upBtn').addEventListener('click', navigateUp);

document.getElementById('scanBtn').addEventListener('click', scanDirectory);

document.getElementById('directoryPath').addEventListener('keypress', function(e) {
    if (e.key === 'Enter') {
        scanDirectory();
    }
});

document.getElementById('historyBtn').addEventListener('click', function() {
    historyModal.show();
});

document.getElementById('browseBtn').addEventListener('click', async function() {
    try {
        const selected = await window.__TAURI__.dialog.select({
            title: '选择要扫描的目录',
            multiple: false,
            directory: true
        });
        if (selected && selected.length > 0) {
            document.getElementById('directoryPath').value = selected[0];
        }
    } catch (error) {
        console.error('选择目录失败:', error);
    }
});

document.getElementById('refreshHistory').addEventListener('click', loadHistory);

document.querySelectorAll('.header-col').forEach(header => {
    header.addEventListener('click', function() {
        const column = this.dataset.sort;
        if (currentSort.column === column) {
            currentSort.direction = currentSort.direction === 'asc' ? 'desc' : 'asc';
        } else {
            currentSort.column = column;
            currentSort.direction = column === 'name' ? 'asc' : 'desc';
        }
        updateSortIndicators();
        if (currentData) {
            renderFileList();
        }
    });
});

function navigateBack() {
    if (navigationIndex > 0) {
        navigationIndex--;
        const path = navigationHistory[navigationIndex];
        navigateToPath(path, false);
    }
}

function navigateForward() {
    if (navigationIndex < navigationHistory.length - 1) {
        navigationIndex++;
        const path = navigationHistory[navigationIndex];
        navigateToPath(path, false);
    }
}

function navigateUp() {
    if (!currentScanPath) return;
    const parentPath = currentScanPath.split(/[\\/]/).slice(0, -1).join('/');
    if (parentPath) {
        navigateToPath(parentPath, true);
    }
}

async function navigateToPath(path, addToHistory) {
    showLoading(true);
    hideError();

    try {
        const data = await invoke('scan_directory', { path: path, forceRefresh: false });
        currentData = data;
        currentScanPath = path;
        document.getElementById('directoryPath').value = path;

        if (addToHistory) {
            navigationHistory = navigationHistory.slice(0, navigationIndex + 1);
            navigationHistory.push(path);
            navigationIndex = navigationHistory.length - 1;
        }

        renderFileList();
        renderTree(rootScanData || data);
        updateStatusBar(data);
        showStats();
        updateNavigationButtons();
    } catch (error) {
        showError('导航失败: ' + error);
    } finally {
        showLoading(false);
    }
}

function updateNavigationButtons() {
    document.getElementById('backBtn').disabled = navigationIndex <= 0;
    document.getElementById('forwardBtn').disabled = navigationIndex >= navigationHistory.length - 1;
    document.getElementById('upBtn').disabled = !currentScanPath || currentScanPath === rootScanData?.path;
}

async function scanDirectory() {
    const path = document.getElementById('directoryPath').value.trim();
    if (!path) {
        showError('请输入目录路径');
        return;
    }

    showLoading(true);
    hideError();

    try {
        const data = await invoke('scan_directory', { path: path, forceRefresh: true });
        rootScanData = data;
        currentData = data;
        currentScanPath = path;
        navigationHistory = [path];
        navigationIndex = 0;

        renderFileList();
        renderTree(data);
        updateStatusBar(data);
        showStats();
        loadHistory();
        updateNavigationButtons();
    } catch (error) {
        showError('扫描失败: ' + error);
    } finally {
        showLoading(false);
    }
}

// 渲染目录树
function renderTree(data) {
    const treeContainer = document.getElementById('treeContainer');
    if (!data) {
        treeContainer.innerHTML = '';
        return;
    }

    // 筛选出文件夹并按路径构建树形结构
    const dirs = data.items.filter(item => item.isDir);
    
    // 构建树形结构
    const treeData = buildTreeStructure(dirs, data.path);
    
    // 渲染树
    const rootName = data.path.split(/[\\/]/).pop() || data.path;
    let treeHtml = `
        <div class="tree-item expanded" data-path="${data.path}" data-is-root="true">
            <div class="tree-content">
                <i class="bi bi-chevron-down tree-arrow"></i>
                <i class="bi bi-folder2-open tree-icon"></i>
                <span class="tree-label" title="${data.path}">${rootName}</span>
            </div>
            <div class="tree-children">
    `;

    // 递归渲染子节点
    treeHtml += renderTreeNodes(treeData, data.path);

    treeHtml += `
            </div>
        </div>
    `;

    treeContainer.innerHTML = treeHtml;

    // 绑定点击事件
    bindTreeEvents();
}

// 构建树形结构
function buildTreeStructure(dirs, rootPath) {
    const tree = {};

    dirs.forEach(dir => {
        const parts = dir.path.split(/[\\/]/).filter(p => p);
        let current = tree;

        parts.forEach((part, index) => {
            if (!current[part]) {
                const relativePath = parts.slice(0, index + 1).join('/');
                current[part] = {
                    name: part,
                    children: {},
                    fullPath: rootPath + '/' + relativePath,
                    relativePath: relativePath,
                    isLeaf: index === parts.length - 1
                };
            }
            current = current[part].children;
        });
    });

    return tree;
}

// 递归渲染树节点
function renderTreeNodes(treeData, parentPath) {
    let html = '';
    const keys = Object.keys(treeData).sort((a, b) => a.localeCompare(b));

    keys.forEach(key => {
        const node = treeData[key];
        const fullPath = node.fullPath;
        const hasChildren = Object.keys(node.children).length > 0;

        html += `
            <div class="tree-item ${hasChildren ? '' : 'leaf'}" data-path="${node.fullPath}" data-full-path="${fullPath}">
                <div class="tree-content">
                    <i class="bi bi-chevron-right tree-arrow ${hasChildren ? '' : 'empty'}"></i>
                    <i class="bi bi-folder tree-icon"></i>
                    <span class="tree-label" title="${node.name}">${node.name}</span>
                </div>
                ${hasChildren ? `<div class="tree-children" style="display: none;">${renderTreeNodes(node.children, fullPath)}</div>` : ''}
            </div>
        `;
    });

    return html;
}

function bindTreeEvents() {
    const treeContainer = document.getElementById('treeContainer');

    treeContainer.querySelectorAll('.tree-content').forEach(item => {
        item.addEventListener('click', async function(e) {
            e.stopPropagation();
            const treeItem = this.closest('.tree-item');
            const fullPath = treeItem.dataset.fullPath;
            const isRoot = treeItem.dataset.isRoot === 'true';
            const arrow = this.querySelector('.tree-arrow');
            const icon = this.querySelector('.tree-icon');
            const children = treeItem.querySelector('.tree-children');

            if (isRoot) return;

            if (treeItem.classList.contains('leaf')) {
                navigateToPath(fullPath, true);
                return;
            }

            if (children) {
                const isExpanded = treeItem.classList.contains('expanded');
                if (isExpanded) {
                    treeItem.classList.remove('expanded');
                    children.style.display = 'none';
                    arrow.classList.remove('bi-chevron-down');
                    arrow.classList.add('bi-chevron-right');
                    icon.classList.remove('bi-folder2-open');
                    icon.classList.add('bi-folder');
                } else {
                    treeItem.classList.add('expanded');
                    children.style.display = 'block';
                    arrow.classList.remove('bi-chevron-right');
                    arrow.classList.add('bi-chevron-down');
                    icon.classList.remove('bi-folder');
                    icon.classList.add('bi-folder2-open');
                }
            }
        });
    });
}

// 渲染文件列表
function renderFileList() {
    const fileList = document.getElementById('fileList');

    if (!currentData || currentData.items.length === 0) {
        fileList.innerHTML = `
            <div class="empty-state">
                <div class="empty-state-icon">
                    <i class="bi bi-folder2-open"></i>
                </div>
                <div>该目录为空或没有可访问的文件</div>
            </div>
        `;
        return;
    }

    // 排序
    let items = [...currentData.items];
    items.sort((a, b) => {
        let aVal = a[currentSort.column];
        let bVal = b[currentSort.column];

        if (currentSort.column === 'name') {
            aVal = a.name || a.path;
            bVal = b.name || b.path;
            return currentSort.direction === 'asc' ? aVal.localeCompare(bVal) : bVal.localeCompare(aVal);
        }

        if (currentSort.column === 'type') {
            aVal = a.isDir ? 0 : 1;
            bVal = b.isDir ? 0 : 1;
            if (aVal !== bVal) {
                return currentSort.direction === 'asc' ? aVal - bVal : bVal - aVal;
            }
            // 类型相同时按名称排序
            aVal = a.name || a.path;
            bVal = b.name || b.path;
            return currentSort.direction === 'asc' ? aVal.localeCompare(bVal) : bVal.localeCompare(aVal);
        }

        if (currentSort.column === 'size') {
            return currentSort.direction === 'asc' ? aVal - bVal : bVal - aVal;
        }

        return 0;
    });

    // 渲染列表
    const html = items.map(item => {
        // 根据文件扩展名选择图标
        let iconClass = 'bi-file-earmark';
        if (item.isDir) {
            iconClass = 'bi-folder-fill';
        } else {
            const ext = item.name.split('.').pop().toLowerCase();
            switch (ext) {
                case 'js':
                case 'ts':
                case 'json':
                case 'html':
                case 'css':
                    iconClass = 'bi-file-earmark-code';
                    break;
                case 'png':
                case 'jpg':
                case 'jpeg':
                case 'gif':
                case 'svg':
                    iconClass = 'bi-file-earmark-image';
                    break;
                case 'pdf':
                    iconClass = 'bi-file-earmark-pdf';
                    break;
                case 'txt':
                case 'md':
                    iconClass = 'bi-file-earmark-text';
                    break;
                case 'zip':
                case 'rar':
                case '7z':
                case 'tar':
                case 'gz':
                    iconClass = 'bi-file-earmark-zip';
                    break;
                case 'exe':
                case 'msi':
                    iconClass = 'bi-file-earmark-binary';
                    break;
            }
        }
        
        const iconColor = item.isDir ? 'color: #ffc107;' : 'color: #6c757d;';
        const type = item.isDir ? '文件夹' : '文件';
        
        return `
            <div class="list-item" data-path="${item.path}">
                <div class="list-item-col name">
                    <div class="item-icon" style="${iconColor}">
                        <i class="bi ${iconClass}"></i>
                    </div>
                    <span class="item-name" title="${item.path}">${item.name || item.path}</span>
                </div>
                <div class="list-item-col size">${item.sizeFormatted}</div>
                <div class="list-item-col type">${type}</div>
            </div>
        `;
    }).join('');

    fileList.innerHTML = html;
}

// 更新排序指示器
function updateSortIndicators() {
    document.querySelectorAll('.sort-indicator').forEach(indicator => {
        indicator.textContent = '';
    });

    const indicator = document.getElementById(`sort-${currentSort.column}`);
    if (indicator) {
        indicator.textContent = currentSort.direction === 'asc' ? '▲' : '▼';
    }
}

// 更新状态栏
function updateStatusBar(data) {
    document.getElementById('statusPath').textContent = data.path;
    document.getElementById('statusItems').textContent = `${data.items.length} 个项目`;
    document.getElementById('statusSize').textContent = data.totalSizeFormatted;
    document.getElementById('statusTime').textContent = `${data.scanTime.toFixed(2)} 秒`;
}

// 加载历史记录
async function loadHistory() {
    try {
        const history = await invoke('get_history');
        const historyList = document.getElementById('historyList');

        if (history.length === 0) {
            historyList.innerHTML = '<p class="text-muted text-center mb-0">暂无历史记录</p>';
            return;
        }

        historyList.innerHTML = history.map(item => `
            <div class="history-item" data-path="${item.path}">
                <div class="d-flex justify-content-between align-items-center">
                    <div class="tree-label">${item.path}</div>
                    <div class="text-end">
                        <div><strong>${item.sizeFormat}</strong></div>
                        <small class="text-muted">${formatTime(item.scanTime)}</small>
                    </div>
                </div>
            </div>
        `).join('');

        // 绑定点击事件
        historyList.querySelectorAll('.history-item').forEach(item => {
            item.addEventListener('click', function() {
                const path = this.dataset.path;
                document.getElementById('directoryPath').value = path;
                historyModal.hide();
                showHistoryItem(path);
            });
        });
    } catch (error) {
        console.error('加载历史记录失败:', error);
    }
}

// 显示历史记录项
async function showHistoryItem(path) {
    showLoading(true);
    hideError();

    try {
        const data = await invoke('get_history_item', { path: path });
        if (data) {
            currentData = data;
            currentScanPath = path;
            renderFileList();
            updateStatusBar(data);
            showStats();
        } else {
            showError('未找到该历史记录');
        }
    } catch (error) {
        showError('获取历史记录失败: ' + error);
    } finally {
        showLoading(false);
    }
}

// 格式化时间
function formatTime(timeString) {
    const date = new Date(timeString);
    const now = new Date();
    const diffMs = now - date;
    const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));
    const diffHours = Math.floor((diffMs % (1000 * 60 * 60 * 24)) / (1000 * 60 * 60));
    const diffMinutes = Math.floor((diffMs % (1000 * 60 * 60)) / (1000 * 60));

    if (diffDays > 0) {
        return `${diffDays}天前`;
    } else if (diffHours > 0) {
        return `${diffHours}小时前`;
    } else if (diffMinutes > 0) {
        return `${diffMinutes}分钟前`;
    } else {
        return '刚刚';
    }
}

// 显示/隐藏加载状态
function showLoading(show) {
    const overlay = document.getElementById('loadingOverlay');
    const scanBtn = document.getElementById('scanBtn');

    if (show) {
        overlay.style.display = 'flex';
        scanBtn.disabled = true;
        scanBtn.innerHTML = '<i class="bi bi-search"></i><span>扫描中...</span>';
    } else {
        overlay.style.display = 'none';
        scanBtn.disabled = false;
        scanBtn.innerHTML = '<i class="bi bi-search"></i><span>扫描</span>';
    }
}

// 显示错误
function showError(message) {
    const toast = document.getElementById('errorToast');
    const messageEl = document.getElementById('errorMessage');

    messageEl.textContent = message;
    toast.style.display = 'flex';

    setTimeout(() => {
        toast.style.display = 'none';
    }, 4000);
}

// 隐藏错误
function hideError() {
    document.getElementById('errorToast').style.display = 'none';
}

// 显示统计分析
function showStats() {
    if (!currentData) return;

    // 销毁旧图表
    if (typeSizeChart) typeSizeChart.destroy();
    if (topItemsChart) topItemsChart.destroy();

    const items = currentData.items;

    // 1. 文件类型分布 (按大小)
    const typeSizeMap = {};
    items.forEach(item => {
        if (!item.isDir) {
            const ext = item.name.split('.').pop().toLowerCase() || '未知';
            typeSizeMap[ext] = (typeSizeMap[ext] || 0) + item.size;
        }
    });

    const sortedTypes = Object.entries(typeSizeMap)
        .sort(([, a], [, b]) => b - a)
        .slice(0, 8);

    const typeLabels = sortedTypes.map(([type]) => type);
    const typeData = sortedTypes.map(([, size]) => size);

    const ctx1 = document.getElementById('typeSizeChart').getContext('2d');
    typeSizeChart = new Chart(ctx1, {
        type: 'pie',
        data: {
            labels: typeLabels,
            datasets: [{
                data: typeData,
                backgroundColor: [
                    '#FF6384', '#36A2EB', '#FFCE56', '#4BC0C0', '#9966FF', '#FF9F40', '#E7E9ED', '#76A346'
                ]
            }]
        },
        options: {
            responsive: true,
            maintainAspectRatio: true,
            plugins: {
                legend: {
                    position: 'bottom',
                    labels: {
                        boxWidth: 12,
                        padding: 8,
                        font: {
                            size: 10
                        }
                    }
                },
                tooltip: {
                    callbacks: {
                        label: function(context) {
                            let label = context.label || '';
                            if (label) {
                                label += ': ';
                            }
                            label += formatSize(context.raw);
                            return label;
                        }
                    }
                }
            }
        }
    });

    // 2. Top 5 大文件/文件夹
    const topItems = [...items].sort((a, b) => b.size - a.size).slice(0, 5);
    const topLabels = topItems.map(item => item.name.length > 15 ? item.name.substring(0, 15) + '...' : item.name);
    const topData = topItems.map(item => item.size);
    const topColors = topItems.map(item => item.isDir ? '#FFCE56' : '#36A2EB');

    const ctx2 = document.getElementById('topItemsChart').getContext('2d');
    topItemsChart = new Chart(ctx2, {
        type: 'bar',
        data: {
            labels: topLabels,
            datasets: [{
                label: '大小',
                data: topData,
                backgroundColor: topColors
            }]
        },
        options: {
            responsive: true,
            maintainAspectRatio: true,
            indexAxis: 'y',
            plugins: {
                legend: {
                    display: false
                },
                tooltip: {
                    callbacks: {
                        label: function(context) {
                            return formatSize(context.raw);
                        }
                    }
                }
            },
            scales: {
                x: {
                    ticks: {
                        callback: function(value) {
                            return formatSize(value);
                        },
                        font: {
                            size: 10
                        }
                    },
                    grid: {
                        display: false
                    }
                },
                y: {
                    ticks: {
                        font: {
                            size: 10
                        }
                    },
                    grid: {
                        display: false
                    }
                }
            }
        }
    });
}

// 辅助函数：格式化大小 (用于图表)
function formatSize(bytes) {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
}
