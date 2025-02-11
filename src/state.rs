use std::collections::HashSet;

use futures::lock::Mutex;
use tokio::sync::broadcast;

pub struct AppState {
    pub user_set: Mutex<HashSet<String>>,
    pub tx: broadcast::Sender<String>,
    pub ip_map: Mutex<HashSet<String>>,
}

impl AppState {
    pub fn new() -> Self {
        let user_set = Mutex::new(HashSet::new());
        let (tx, _rx) = broadcast::channel(100);
        let ip_map = Mutex::new(HashSet::new());
        Self {
            user_set,
            tx,
            ip_map,
        }
    }

    pub async fn try_register_username(&self, string: &mut String, name: &str) -> bool {
        let mut user_set = self.user_set.lock().await;

        if user_set.contains(name) {
            return false;
        }

        user_set.insert(name.to_owned());
        string.push_str(name);
        true
    }

    pub async fn try_register_ip(&self, ip: &str, is_already_contained: &mut bool) {
        let mut ip_map = self.ip_map.lock().await;
        if ip_map.contains(ip) {
            *is_already_contained = true;
        } else {
            ip_map.insert(ip.to_string());
        }
    }

    pub async fn unregister_ip(&self, ip: &str) {
        let mut ip_map = self.ip_map.lock().await;
        ip_map.remove(ip);
    }
}
