use std::sync::Arc;
use dashmap::DashMap;
use tokio::sync::mpsc;
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
}