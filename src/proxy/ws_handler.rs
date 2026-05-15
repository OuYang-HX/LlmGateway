/// WebSocket proxy support for LLM streaming
use axum::{
    extract::{ws::{Message, WebSocket, WebSocketUpgrade}, State},
    response::IntoResponse,
};
use crate::AppState;
use futures::{SinkExt, StreamExt};

/// WebSocket upgrade handler for LLM proxy
pub async fn ws_proxy_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws_proxy(socket, state))
}

async fn handle_ws_proxy(mut socket: WebSocket, state: AppState) {
    // Read the first message which should contain the LLM request
    let first_msg = socket.recv().await;
    
    let request_data = match first_msg {
        Some(Ok(Message::Text(text))) => text,
        Some(Ok(Message::Binary(data))) => String::from_utf8_lossy(&data).to_string(),
        Some(Ok(Message::Ping(_))) => {
            // Ignore ping, wait for actual data
            let next = socket.recv().await;
            match next {
                Some(Ok(Message::Text(text))) => text,
                _ => return,
            }
        }
        _ => return,
    };

    // Parse the request to get model, stream flag, etc.
    let request: serde_json::Value = match serde_json::from_str(&request_data) {
        Ok(v) => v,
        Err(_) => {
            let _ = socket.send(Message::Text(
                serde_json::json!({"error": "Invalid JSON request"}).to_string()
            ).await);
            return;
        }
    };

    // Extract API key from the request or use a default
    let api_key = request.get("api_key")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if api_key.is_empty() {
        let _ = socket.send(Message::Text(
            serde_json::json!({"error": "Missing API key"}).to_string()
        ).await);
        return;
    }

    // Validate API key
    let key_hash = crate::utils::sha256_hash(api_key);
    let api_key_row = match state.db.get_api_key_by_hash(&key_hash).await {
        Ok(Some(row)) => row,
        Ok(None) => {
            let _ = socket.send(Message::Text(
                serde_json::json!({"error": "Invalid API key"}).to_string()
            ).await);
            return;
        }
        Err(_) => {
            let _ = socket.send(Message::Text(
                serde_json::json!({"error": "Internal error"}).to_string()
            ).await);
            return;
        }
    };

    // Determine allowed providers
    let allowed_providers: Option<Vec<String>> = api_key_row.allowed_providers
        .as_ref()
        .and_then(|s| serde_json::from_str(s).ok());

    // Select provider
    let provider = match state.proxy.select_provider(allowed_providers.as_deref()).await {
        Ok(p) => p,
        Err(_) => {
            let _ = socket.send(Message::Text(
                serde_json::json!({"error": "No provider available"}).to_string()
            ).await);
            return;
        }
    };

    // Get auth header
    let (auth_header_name, auth_header_value) = match state.auth_manager.get_auth_header(&provider).await {
        Ok(h) => h,
        Err(_) => {
            let _ = socket.send(Message::Text(
                serde_json::json!({"error": "Auth error"}).to_string()
            ).await);
            return;
        }
    };

    // Forward request to provider via HTTP (WebSocket → HTTP bridge)
    let target_url = format!("{}{}", provider.base_url.trim_end_matches('/'), "/chat/completions");
    
    // Build request body (remove api_key from the forwarded request)
    let mut forward_body = request.clone();
    forward_body.as_object_mut().map(|m| m.remove("api_key"));
    forward_body.as_object_mut().map(|m| m.insert("stream".to_string(), serde_json::Value::Bool(true)));

    let http_response = state.proxy.http_client()
        .post(&target_url)
        .header(&auth_header_name, &auth_header_value)
        .header("content-type", "application/json")
        .json(&forward_body)
        .send()
        .await;

    match http_response {
        Ok(resp) => {
            if resp.status().is_success() {
                // Stream the SSE response back through WebSocket
                let mut stream = resp.bytes_stream();
                while let Some(chunk) = stream.next().await {
                    match chunk {
                        Ok(bytes) => {
                            let text = String::from_utf8_lossy(&bytes).to_string();
                            // Split into SSE lines and send each as a WS message
                            for line in text.lines() {
                                if line.starts_with("data: ") && !line.contains("[DONE]") {
                                    let _ = socket.send(Message::Text(line.to_string())).await;
                                }
                            }
                        }
                        Err(_) => break,
                    }
                }
                let _ = socket.send(Message::Text("data: [DONE]".to_string())).await;
            } else {
                let status = resp.status().as_u16();
                let body = resp.text().await.unwrap_or_default();
                let _ = socket.send(Message::Text(
                    serde_json::json!({"error": "Upstream error", "status": status, "body": body}).to_string()
                ).await);
            }
        }
        Err(e) => {
            let _ = socket.send(Message::Text(
                serde_json::json!({"error": format!("Connection error: {}", e)}).to_string()
            ).await);
        }
    }
}