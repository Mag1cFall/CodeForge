use serde::de::DeserializeOwned;
use tauri::ipc::InvokeBody;
use tauri::test::{
    get_ipc_response, mock_builder, mock_context, noop_assets, MockRuntime, INVOKE_KEY,
};
use tauri::WebviewWindowBuilder;

use crate::build_app;

fn invoke<T: DeserializeOwned>(
    webview: &tauri::WebviewWindow<MockRuntime>,
    cmd: &str,
    payload: serde_json::Value,
) -> T {
    let response = get_ipc_response(
        webview,
        tauri::webview::InvokeRequest {
            cmd: cmd.into(),
            callback: tauri::ipc::CallbackFn(0),
            error: tauri::ipc::CallbackFn(1),
            url: "http://tauri.localhost".parse().unwrap(),
            body: InvokeBody::Json(payload),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.to_string(),
        },
    )
    .unwrap_or_else(|error| panic!("invoke {cmd} failed: {error:?}"));

    response.deserialize::<T>().unwrap()
}

fn create_webview() -> tauri::WebviewWindow<MockRuntime> {
    let app = build_app(mock_builder())
        .build(mock_context(noop_assets()))
        .expect("mock app should build");
    WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("mock webview should build")
}

#[test]
#[ignore]
fn full_chain_invoke_live() {
    let endpoint =
        std::env::var("CODEFORGE_LIVE_LLM_ENDPOINT").expect("CODEFORGE_LIVE_LLM_ENDPOINT required");
    let api_key =
        std::env::var("CODEFORGE_LIVE_LLM_API_KEY").expect("CODEFORGE_LIVE_LLM_API_KEY required");
    let model =
        std::env::var("CODEFORGE_LIVE_LLM_MODEL").expect("CODEFORGE_LIVE_LLM_MODEL required");
    let webview = create_webview();

    let agents: serde_json::Value = invoke(&webview, "agent_list", serde_json::json!({}));
    let agent_id = agents[0]["id"]
        .as_str()
        .expect("agent id should exist")
        .to_string();
    println!("IPC_AGENT_LIST={}", agents);

    let provider: serde_json::Value = invoke(
        &webview,
        "provider_create",
        serde_json::json!({
            "config": {
                "name": "Live OpenAI Compatible",
                "providerType": "openAiCompatible",
                "endpoint": endpoint,
                "apiKey": api_key,
                "model": model,
                "models": [model],
                "enabled": true,
                "isDefault": true,
                "headers": {}
            }
        }),
    );
    println!("IPC_PROVIDER_CREATE={}", provider);

    let session: serde_json::Value = invoke(
        &webview,
        "session_create",
        serde_json::json!({ "agentId": agent_id }),
    );
    let session_id = session["id"]
        .as_str()
        .expect("session id should exist")
        .to_string();
    println!("IPC_SESSION_CREATE={}", session);

    let repo_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repo root should exist")
        .to_path_buf();
    let lib_rs = repo_root.join("src-tauri").join("src").join("lib.rs");

    let tool_output: String = invoke(
        &webview,
        "tool_execute",
        serde_json::json!({ "name": "read_file", "args": { "path": lib_rs.display().to_string() } }),
    );
    println!(
        "IPC_TOOL_EXECUTE={}",
        tool_output.lines().take(3).collect::<Vec<_>>().join(" | ")
    );

    let _: () = invoke(
        &webview,
        "chat_send",
        serde_json::json!({
            "sessionId": session_id,
            "message": format!("请先调用 read_file 读取路径 {}，再只回复文件里注册的第一个 command 名称。", lib_rs.display())
        }),
    );
    let messages: serde_json::Value = invoke(
        &webview,
        "session_messages",
        serde_json::json!({ "id": session_id }),
    );
    println!("IPC_CHAT_MESSAGES={}", messages);

    let _: () = invoke(
        &webview,
        "knowledge_index",
        serde_json::json!({ "path": repo_root.display().to_string() }),
    );
    let knowledge: serde_json::Value = invoke(
        &webview,
        "knowledge_search",
        serde_json::json!({ "query": "agent loop", "topK": 3 }),
    );
    println!("IPC_KNOWLEDGE_SEARCH={}", knowledge);

    let _: () = invoke(
        &webview,
        "project_review",
        serde_json::json!({ "path": repo_root.display().to_string(), "sandbox": false }),
    );
    let logs: serde_json::Value = invoke(&webview, "log_list", serde_json::json!({ "limit": 10 }));
    println!("IPC_LOG_LIST={}", logs);
}
