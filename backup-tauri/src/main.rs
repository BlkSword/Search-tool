use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Json},
    routing::{get, post},
    Router,
};
use search_tool::scan::{scan_directory, HistoryItem, ScanResult};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::{cors::CorsLayer, services::ServeDir};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// 历史记录存储
#[derive(Clone)]
struct AppState {
    history: Arc<RwLock<Vec<HistoryItem>>>,
}

#[derive(Deserialize)]
struct ScanRequest {
    path: String,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "search_tool=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // 初始化状态
    let state = AppState {
        history: Arc::new(RwLock::new(Vec::new())),
    };

    // 构建路由
    let app = Router::new()
        .route("/", get(index_handler))
        .route("/api/scan", post(scan_handler))
        .route("/api/history", get(history_handler))
        .route("/api/history-item", post(history_item_handler))
        .nest_service("/static", ServeDir::new("static"))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080")
        .await
        .unwrap();

    tracing::info!("服务器启动在 http://localhost:8080");
    axum::serve(listener, app).await.unwrap();
}

// 主页处理器
async fn index_handler() -> Html<String> {
    let html = tokio::fs::read_to_string("templates/index.html")
        .await
        .unwrap_or_else(|_| {
            r#"<!DOCTYPE html>
<html>
<head><title>Error</title></head>
<body><h1>Template not found</h1></body>
</html>"#
                .to_string()
        });
    Html(html)
}

// 扫描处理器
async fn scan_handler(
    State(state): State<AppState>,
    Json(payload): Json<ScanRequest>,
) -> Result<Json<ScanResult>, (StatusCode, Json<ErrorResponse>)> {
    let path = payload.path.trim();

    if path.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "请提供有效的目录路径".to_string(),
            }),
        ));
    }

    match scan_directory(path).await {
        Ok(mut result) => {
            // 添加到历史记录
            let history_item = HistoryItem {
                path: path.to_string(),
                scan_time: chrono::Utc::now(),
                total_size: result.total_size,
                size_format: result.total_size_formatted.clone(),
                items: result.items.clone(),
            };

            // 保存到历史记录
            let mut history = state.history.write().await;
            history.push(history_item);

            // 保持历史记录在合理范围内（最多保存50条）
            if history.len() > 50 {
                history.remove(0);
            }

            // 更新结果中的路径为规范路径
            result.path = path.to_string();

            Ok(Json(result))
        }
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}

// 历史记录处理器
async fn history_handler(State(state): State<AppState>) -> Json<Vec<HistoryItem>> {
    let history = state.history.read().await;
    // 返回逆序（最新的在前）
    let reversed: Vec<HistoryItem> = history.iter().rev().cloned().collect();
    Json(reversed)
}

// 历史记录详情处理器
async fn history_item_handler(
    State(state): State<AppState>,
    Json(payload): Json<ScanRequest>,
) -> Result<Json<ScanResult>, (StatusCode, Json<ErrorResponse>)> {
    let path = &payload.path;

    let history = state.history.read().await;

    // 查找最新的匹配历史记录
    for item in history.iter().rev() {
        if item.path == *path {
            let result = ScanResult {
                items: item.items.clone(),
                total_size: item.total_size,
                total_size_formatted: item.size_format.clone(),
                scan_time: 0.0, // 历史记录没有扫描时间
                path: item.path.clone(),
            };
            return Ok(Json(result));
        }
    }

    Err((
        StatusCode::NOT_FOUND,
        Json(ErrorResponse {
            error: "未找到该历史记录".to_string(),
        }),
    ))
}
