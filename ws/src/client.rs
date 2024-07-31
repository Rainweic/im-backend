use axum::extract::ws::{Message, WebSocket};
use futures::stream::SplitSink;
use futures::SinkExt;
use std::sync::Arc;
use tokio::sync::RwLock;

type ClientSender = Arc<RwLock<SplitSink<WebSocket, Message>>>;

pub struct Client {
    pub sender: ClientSender,
    pub user_id: String,
    pub platform_id: String,
}

impl Client {
    pub async fn send_text(&self, msg: String) -> Result<(), axum::Error> {
        self.sender.write().await.send(Message::Text(msg)).await
    }

    #[allow(dead_code)]
    pub async fn send_binary(&self, msg: Vec<u8>) -> Result<(), axum::Error> {
        self.sender.write().await.send(Message::Binary(msg)).await
    }
}
