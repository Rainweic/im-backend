use std::sync::Arc;
use std::sync::mpsc::SendError;
use dashmap::DashMap;
use tokio::sync::mpsc;
use tracing::{debug, error, info};
use abi::message::Msg;
use crate::client::Client;

type UserID = String;
type PlatformID = String;
/// 客户连接池
type Hub = Arc<DashMap<UserID, DashMap<PlatformID, Client>>>;

#[derive(Clone)]
pub struct Manager {
    /// 用来接收websocket收到的消息
    tx: mpsc::Sender<Msg>,
    pub hub: Hub,
}

impl Manager {
    pub fn new(tx: mpsc::Sender<Msg>) -> Self {
        Self {
            tx,
            hub: Arc::new(DashMap::new())
        }
    }

    /// 注册
    pub async fn register(&mut self, user_id: UserID, client: Client) {
        let entry = self.hub.entry(user_id.clone())
            .or_insert_with(DashMap::new);
        entry.insert(client.platform_id.clone(), client);
    }

    /// 注销
    pub async fn unregister(&mut self, user_id: UserID, platform_id: PlatformID) {
        if let Some(user_clients) = self.hub.get_mut(&user_id) {
            user_clients.remove(&platform_id);
        }
    }

    /// 消息发送给所有客户端
    async fn send_msg_to_clients(&self, clients: &DashMap<PlatformID, Client>, msg: &Msg) {
        for client in clients.iter() {
            let content = match serde_json::to_string(msg) {
                Ok(res) => res,
                Err(_) => {
                    error!("msg serialize error");
                    return;
                }
            };
            if let Err(e) = client.value().send_text(content).await {
                error!("msg send error: {:?}", e);
            }
        }
    }

    /// 发送消息到指定user
    pub async fn send_single_msg(&self, user_id: &UserID, msg: &Msg) {
        if let Some(clients) = self.hub.get(user_id) {
            self.send_msg_to_clients(&clients, msg).await
        }
    }

    pub async fn run(&mut self, mut receiver: mpsc::Receiver<Msg>) {
        info!("manager start");
        while let Some(message) = receiver.recv().await {
            debug!("receive message: {:?}", message);
            self.send_single_msg(&message.receiver_id, &message).await;
        }
    }

    pub async fn broadcast(&self, msg: Msg) -> Result<(), tokio::sync::mpsc::error::SendError<Msg>> {
        self.tx.send(msg).await
    }
}