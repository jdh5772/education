use std::sync::Arc;

use crate::state::AppState;

pub struct DisconnectGuard {
    ip: String,
    state: Arc<AppState>,
}

impl DisconnectGuard {
    pub fn new(ip: String, state: Arc<AppState>) -> Self {
        Self { ip, state }
    }
}

impl Drop for DisconnectGuard {
    fn drop(&mut self) {
        let ip = self.ip.clone();
        let state = self.state.clone();

        tokio::spawn(async move {
            state.unregister_ip(&ip).await;
        });
    }
}
