use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        ConnectInfo, State,
    },
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use futures::{sink::SinkExt, stream::StreamExt};
use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use tokio::sync::broadcast;

struct AppState {
    user_set: Mutex<HashSet<String>>,
    tx: broadcast::Sender<String>,
    ip_map: Mutex<HashMap<String, bool>>,
}

async fn websocket_handler(
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| websocket(socket, state, addr))
}

async fn websocket(stream: WebSocket, state: Arc<AppState>, addr: SocketAddr) {
    let addr = addr.to_string();
    let mut vec = addr.split(":");
    let ip = vec.next().unwrap();
    let mut is_already_contained = false;

    let (mut sender, mut receiver) = stream.split();

    try_register_ip(&state, ip, &mut is_already_contained).await;

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
            let is_registered = try_register_username(&state, &mut username, name);

            if name.is_empty() {
                let _ = sender
                    .send(Message::Text(String::from(
                        "공백은 사용 불가 !!! 새로고침 하세요.",
                    )))
                    .await;
                unregister_ip(&state, ip).await;

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
            unregister_ip(&state, ip).await;
        },
        _ = &mut recv_task => {
            send_task.abort();
            unregister_ip(&state, ip).await;
        },

    };

    let msg = format!("{username} left.");
    let _ = state.tx.send(msg);

    state.user_set.lock().unwrap().remove(&username);
    state.ip_map.lock().unwrap().remove(ip);
}

fn try_register_username(state: &AppState, string: &mut String, name: &str) -> bool {
    let mut user_set = state.user_set.lock().unwrap();

    if user_set.contains(name) {
        return false;
    }

    user_set.insert(name.to_owned());
    string.push_str(name);
    true
}

async fn index() -> Html<&'static str> {
    Html(std::include_str!("../chat.html"))
}

async fn try_register_ip(state: &AppState, ip: &str, is_already_contained: &mut bool) {
    let mut ip_map = state.ip_map.lock().unwrap();
    if ip_map.contains_key(ip) {
        *is_already_contained = true;
    } else {
        ip_map.insert(ip.to_string(), true);
    }
}

async fn unregister_ip(state: &AppState, ip: &str) {
    let mut ip_map = state.ip_map.lock().unwrap();
    ip_map.remove(ip);
}

#[tokio::main]
async fn main() {
    let user_set = Mutex::new(HashSet::new());
    let (tx, _rx) = broadcast::channel(100);
    let ip_map = Mutex::new(HashMap::new());

    let app_state = Arc::new(AppState {
        user_set,
        tx,
        ip_map,
    });

    let app = Router::new()
        .route("/", get(index))
        .route("/websocket", get(websocket_handler))
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}
