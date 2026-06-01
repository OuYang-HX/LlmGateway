use llm_gateway::api::{build_test_request, TestRequest};
use llm_gateway::db::ProviderRow;

fn make_provider(api_type: &str, base_url: &str) -> ProviderRow {
    ProviderRow {
        id: "p".to_string(), name: "p".to_string(),
        base_url: base_url.to_string(),
        api_type: api_type.to_string(), auth_type: "api_key".to_string(),
        api_key: Some("sk-test".to_string()),
        token_url: None, token_username: None, token_password: None,
        token_request_method: None, token_content_type: None,
        token_username_field: None, token_password_field: None,
        token_body_template: None, token_extra_headers: None, token_cookies: None,
        token_field: "token".to_string(), refresh_token_field: "refreshToken".to_string(),
        token_header_field: "Authorization".to_string(), token_header_prefix: "Bearer ".to_string(),
        token_expiry_seconds: 86400, current_token: None, current_refresh_token: None, token_expires_at: None,
        is_active: true, weight: 1, bypass_proxy: false,
        response_content_path: String::new(), response_reasoning_path: String::new(),
        chart_color: None, subscription_start: None, mock_mode: false,
        group_id: None,
        created_at: String::new(), updated_at: String::new(),
    }
}

fn header<'a>(req: &'a TestRequest, name: &str) -> Option<&'a str> {
    req.headers.iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

// ==================== build_test_request ====================

#[test]
fn test_build_test_request_openai_uses_chat_completions() {
    let p = make_provider("openai", "https://api.example.com/v1");
    let req = build_test_request(&p, "gpt-4o");
    assert_eq!(req.url, "https://api.example.com/v1/chat/completions");
    assert!(header(&req, "anthropic-version").is_none(),
        "OpenAI request must NOT include anthropic-version header");
    assert_eq!(req.body["model"], "gpt-4o");
    assert_eq!(req.body["max_tokens"], 1);
}

#[test]
fn test_build_test_request_anthropic_uses_messages_and_version_header() {
    let p = make_provider("anthropic", "https://api.minimaxi.com/anthropic/v1");
    let req = build_test_request(&p, "MiniMax-M2.7-highspeed");
    assert_eq!(req.url, "https://api.minimaxi.com/anthropic/v1/messages",
        "Anthropic provider must use /messages path, not /chat/completions");
    let av = header(&req, "anthropic-version");
    assert!(av.is_some(), "Anthropic request MUST include anthropic-version header");
    assert_eq!(av.unwrap(), "2023-06-01");
    assert_eq!(req.body["model"], "MiniMax-M2.7-highspeed");
    assert_eq!(req.body["max_tokens"], 1);
}

#[test]
fn test_build_test_request_strips_trailing_slash_in_base_url() {
    let p = make_provider("openai", "https://api.example.com/v1/");
    let req = build_test_request(&p, "m");
    assert_eq!(req.url, "https://api.example.com/v1/chat/completions",
        "Trailing slash on base_url must not produce // in target URL");
}

#[test]
fn test_build_test_request_strips_trailing_slash_anthropic() {
    let p = make_provider("anthropic", "https://api.example.com/v1/");
    let req = build_test_request(&p, "m");
    assert_eq!(req.url, "https://api.example.com/v1/messages");
}

#[test]
fn test_build_test_request_unknown_api_type_falls_back_to_openai() {
    // Forward-compat: unknown api_type defaults to OpenAI-style path
    let p = make_provider("custom-future-type", "https://api.example.com/v1");
    let req = build_test_request(&p, "m");
    assert_eq!(req.url, "https://api.example.com/v1/chat/completions");
    assert!(header(&req, "anthropic-version").is_none());
}

#[test]
fn test_build_test_request_message_body_shape() {
    let p = make_provider("anthropic", "https://api.example.com/v1");
    let req = build_test_request(&p, "claude-x");
    // Body must include `model`, `messages` (with one user msg), and `max_tokens`
    assert!(req.body["messages"].is_array());
    assert_eq!(req.body["messages"].as_array().unwrap().len(), 1);
    assert_eq!(req.body["messages"][0]["role"], "user");
    assert!(req.body["messages"][0]["content"].is_string());
}
