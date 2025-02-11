use std::{net::SocketAddr, sync::Arc};

use axum::extract::ws::{Message, WebSocket};
use futures::{SinkExt, StreamExt};

use crate::state::AppState;

pub async fn websocket(stream: WebSocket, state: Arc<AppState>, addr: SocketAddr) {
    let addr = addr.to_string();
    let mut vec = addr.split(":");
    let ip = vec.next().unwrap();
    let mut is_already_contained = false;

    let (mut sender, mut receiver) = stream.split();

    state.try_register_ip(ip, &mut is_already_contained).await;

    if is_already_contained {
        let _ = sender
            .send(Message::Text(String::from("이미 접속중입니다!!")))
            .await;
        return;
    }

    let mut username = String::new();
    while let Some(Ok(message)) = receiver.next().await {
        if let Message::Text(name) = message {
            let name = name.trim();
            let is_registered = state.try_register_username(&mut username, name).await;

            if name.is_empty() {
                let _ = sender
                    .send(Message::Text(String::from(
                        "공백은 사용 불가 !!! 새로고침 하세요.",
                    )))
                    .await;
                state.unregister_ip(ip).await;

                return;
            } else if !is_registered {
                let _ = sender
                    .send(Message::Text(String::from(
                        "누군가 같은 닉네임 사용 중 !!! 새로고침 하세요.",
                    )))
                    .await;
                return;
            } else {
                break;
            }
        }
    }

    let mut rx = state.tx.subscribe();

    let msg = format!("{username} joined.");
    let _ = state.tx.send(msg);

    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if sender.send(Message::Text(msg)).await.is_err() {
                break;
            }
        }
    });

    let tx = state.tx.clone();
    let name = username.clone();

    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(Message::Text(text))) = receiver.next().await {
            let _ = tx.send(format!("{name}: {text}"));
        }
    });

    tokio::select! {
        _ = &mut send_task => {
            recv_task.abort();
            state.unregister_ip( ip).await;
        },
        _ = &mut recv_task => {
            send_task.abort();
            state.unregister_ip(ip).await;
        },

    };

    let msg = format!("{username} left.");
    let _ = state.tx.send(msg);

    state.user_set.lock().await.remove(&username);
    state.ip_map.lock().await.remove(ip);
}
