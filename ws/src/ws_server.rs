use crate::client::Client;
use crate::manager::Manager;
use axum::extract::ws::{Message, WebSocket};
use axum::extract::{Path, State, WebSocketUpgrade};
use axum::response::IntoResponse;
use futures::{SinkExt, StreamExt};
use std::sync::Arc;
use std::time::Duration;
use axum::Router;
use axum::routing::get;
use tokio::sync::{mpsc, RwLock};
use tracing::log::{error, info};
use abi::config::Config;

pub const HEART_BEAT_INTERVAL: u64 = 30;

#[derive(Clone)]
pub struct AppState {
    manager: Manager,
}

pub async fn websocket(user_id: String, pointer_id: String, ws: WebSocket, app_state: AppState) {
    tracing::debug!(
        "client {} connected, user id : {}",
        user_id.clone(),
        pointer_id.clone()
    );

    let mut hub = app_state.manager.clone();

    let (ws_tx, mut ws_rx) = ws.split();
    let shared_tx = Arc::new(RwLock::new(ws_tx));
    let client = Client {
        user_id: user_id.clone(),
        platform_id: pointer_id.clone(),
        sender: shared_tx.clone(),
    };
    hub.register(user_id.clone(), client).await;

    let cloned_tx = shared_tx.clone();
    // 开另一个线程 每30s进行ping一次（心跳）
    let mut ping_task = tokio::spawn(async move {
        loop {
            if let Err(e) = cloned_tx
                .write()
                .await
                .send(Message::Ping(Vec::new()))
                .await
            {
                error!("send ping error：{:?}", e);
                // break this task, it will end this conn
                break;
            }
            tokio::time::sleep(Duration::from_secs(HEART_BEAT_INTERVAL)).await;
        }
    });

    let cloned_hub = hub.clone();
    let shared_tx = shared_tx.clone();
    let mut rec_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_rx.next().await {
            match msg {
                Message::Text(text) => {
                    let result = serde_json::from_str(&text);
                    if result.is_err() {
                        error!("deserialize error: {:?}； source: {text}", result.err());
                        continue;
                    }

                    if cloned_hub.broadcast(result.unwrap()).await.is_err() {
                        break;
                    }
                }
                Message::Ping(_) => {
                    if let Err(e) = shared_tx
                        .write()
                        .await
                        .send(Message::Pong(Vec::new()))
                        .await
                    {
                        error!("reply ping error : {:?}", e);
                        break;
                    }
                }
                Message::Close(info) => {
                    if let Some(info) = info {
                        tracing::warn!("client closed {}", info.reason);
                    }
                    break;
                }
                _ => {}
            }
        }
    });

    tokio::select! {
        _ = (&mut ping_task) => rec_task.abort(),
        _ = (&mut rec_task) => ping_task.abort(),
    }

    // lost the connection, remove the client from hub
    hub.unregister(user_id, pointer_id).await;
    tracing::debug!("client thread exit {}", hub.hub.iter().count());
}


pub async fn websocket_handler(
    Path((user_id, pointer_id)): Path<(String, String)>,
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| websocket(user_id, pointer_id, socket, state))
}


pub async fn start(config: Config) {
    // 创建通道，用来从websocket连接中向manager发送消息。
    let (tx, rx) = mpsc::channel(1024);
    let manager = Manager::new(tx);
    let mut cloned_manager = manager.clone();
    tokio::spawn(async move {
        cloned_manager.run(rx).await;
    });
    let app_state = AppState {
        manager: manager.clone(),
    };

    // 定义一个处理WebSocket连接的路由。
    let router = Router::new()
        .route("/ws/:user_id/conn/:pointer_id", get(websocket_handler))
        .with_state(app_state);
    let addr = format!("{}:{}", config.websocket.host, config.websocket.port);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    info!("start websocket server on {}", addr);
    axum::serve(listener, router).await.unwrap();
}
