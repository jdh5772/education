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
    ip_map: Mutex<HashMap<String, String>>,
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
    let port = vec.next().unwrap();
    let mut is_contained = false;

    check_ip(&state, ip, port, &mut is_contained).await;

    let (mut sender, mut receiver) = stream.split();

    if is_contained {
        let _ = sender
            .send(Message::Text(String::from("이미 접속중입니다!!")).into())
            .await;
        return;
    }

    let mut username = String::new();
    while let Some(Ok(message)) = receiver.next().await {
        if let Message::Text(name) = message {
            check_username(&state, &mut username, &name);

            if !username.is_empty() {
                break;
            } else {
                let _ = sender
                    .send(
                        Message::Text(String::from(
                            "누군가 같은 닉네임 사용 중 !!! 새고로침 하세요.",
                        ))
                        .into(),
                    )
                    .await;

                return;
            }
        }
    }

    let mut rx = state.tx.subscribe();

    let msg = format!("{username} joined.");
    let _ = state.tx.send(msg);

    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if sender.send(Message::Text(msg).into()).await.is_err() {
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
        _ = &mut send_task => recv_task.abort(),
        _ = &mut recv_task => send_task.abort(),
    };

    let msg = format!("{username} left.");
    let _ = state.tx.send(msg);

    state.user_set.lock().unwrap().remove(&username);
    state.ip_map.lock().unwrap().remove(ip);
}

fn check_username(state: &AppState, string: &mut String, name: &str) {
    let mut user_set = state.user_set.lock().unwrap();

    if !user_set.contains(name) {
        user_set.insert(name.to_owned());

        string.push_str(name);
    }
}

async fn index() -> Html<&'static str> {
    Html(std::include_str!("../chat.html"))
}

async fn check_ip(state: &AppState, ip: &str, port: &str, is_contained: &mut bool) {
    let mut state = state.ip_map.lock().unwrap();
    if state.contains_key(ip) {
        *is_contained = true;
    } else {
        state.insert(ip.to_string(), port.to_string());
    }
}
