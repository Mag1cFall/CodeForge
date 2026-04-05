# CodeForge (炼码) — 后端 Rust 实现指南

> **⚠️ 核心原则**: AI Agent 领域演进极快。**任何 Agent 相关的实现（工具调用、MCP、Agent 编排、Skill 等）都必须在 `C:\Users\l\Desktop\数据挖掘\ref\` 目录下找到大量参考和正确实现后才能编码**。不要凭知识库编写，必须读源码。

---

## 0. 项目概况

- **技术栈**: Rust (Tauri v2) + React 19 + TypeScript
- **前端已完成**: 11 个页面骨架全部就绪，使用 mock 数据
- **你的任务**: 实现后端 Rust 逻辑，通过 Tauri IPC commands 提供真实数据
- **GitHub**: https://github.com/Mag1cFall/CodeForge

## 1. 参考项目清单

| 项目 | 路径 | 技术栈 | 参考重点 |
|---|---|---|---|
| **AstrBot ⭐⭐⭐** | `ref/astrbot/` | Python | Agent 运行时、MCP 客户端、工具系统、Skill、知识库、Provider |
| **OpenClaw ⭐⭐⭐** | `ref/openclaw/` | TypeScript | Gateway 架构、MCP Server、多通道、Skills、安全模型 |
| **Claurst** | `ref/claurst/` | Rust | Rust 实现参考、40+ 工具定义、权限系统、BUDDY 系统 |
| **OpenCode** | `ref/opencode/` | Go | 工具调用循环、流式响应、Agent loop |
| **Oh-My-OpenAgent** | `ref/oh-my-openagent/` | Python | 多 Agent 协作模式、Hash-Anchored Editing |

---

## 2. 实现任务 (按优先级排列)

### Task 1: LLM Provider 抽象层

**目标**: 实现 OpenAI/Anthropic/DeepSeek 等 Provider 的统一接口。

**前端接口**: 前端通过 `invoke("chat_send", { messages, providerId })` 调用。

**必须参考的文件** (先读再写):
```
ref/astrbot/astrbot/core/provider/sources/openai_source.py     (49KB, 最完整的 OpenAI 实现)
ref/astrbot/astrbot/core/provider/sources/anthropic_source.py  (31KB, Anthropic 实现)
ref/astrbot/astrbot/core/provider/manager.py                    (36KB, Provider 管理器)
ref/astrbot/astrbot/core/provider/register.py                   (Provider 注册机制)
ref/astrbot/astrbot/core/provider/entities.py                   (16KB, 数据实体定义)
```

**实现要点**:
1. 定义 `LlmProvider` trait (参考 AstrBot 的 provider.py 基类)
2. 实现 `openai_compatible.rs` — 覆盖 OpenAI / DeepSeek / Ollama
3. 实现 `anthropic.rs` — Anthropic 格式
4. 流式响应: 参考 AstrBot 的 SSE 处理，用 Tauri `emit()` 推送 chunk
5. Provider 配置持久化到 SQLite

**Rust 文件位置**: `src-tauri/src/llm/`

---

### Task 2: Tool Calling 工具系统

**目标**: 定义工具 Schema，实现内置工具，支持工具注册和执行。

**前端接口**: `invoke("tool_list")`, `invoke("tool_execute", { name, args })`

**必须参考的文件**:
```
ref/astrbot/astrbot/core/agent/tool.py                          (13KB, ToolSchema/FunctionTool/ToolSet 定义)
ref/astrbot/astrbot/core/astr_agent_tool_exec.py                (28KB, 工具执行引擎)
ref/astrbot/astrbot/core/provider/func_tool_manager.py          (35KB, 功能工具管理)
ref/claurst/README.md                                            (Line 353-396, 40+ 工具列表)
```

**关键**: ToolSet 类的 `openai_schema()` / `anthropic_schema()` 方法 — 这是让工具调用与不同 LLM 格式兼容的核心。必须从 AstrBot 的 tool.py 学习转换逻辑。

**内置工具清单**:
- `read_file` / `write_file` / `list_directory` (文件操作)
- `search_code` / `grep_pattern` (搜索)
- `run_shell` (Shell 执行, 需权限确认)
- `analyze_ast` (tree-sitter, 可选)
- `find_code_smells` / `suggest_refactor` (调 LLM)

**Rust 文件位置**: `src-tauri/src/tools/`

---

### Task 3: Agent 运行循环

**目标**: 实现 Agent 的核心 loop: 用户消息 → LLM 调用 → 工具调用 → 反馈 → 循环

**必须参考的文件**:
```
ref/astrbot/astrbot/core/astr_main_agent.py                     (52KB, 最核心! 主 Agent 引擎)
ref/astrbot/astrbot/core/astr_agent_run_util.py                 (19KB, Agent 运行工具函数)
ref/astrbot/astrbot/core/agent/agent.py                          (Agent 数据结构: name/instructions/tools/hooks)
ref/astrbot/astrbot/core/agent/hooks.py                          (Agent 生命周期钩子)
ref/astrbot/astrbot/core/agent/runners/                          (各种 Runner 实现)
ref/astrbot/astrbot/core/subagent_orchestrator.py                (3.8KB, 子 Agent 编排)
```

**核心 Loop (伪代码)**:
```
loop {
    response = llm.chat(messages, tools)?;
    if response.has_tool_calls() {
        for call in response.tool_calls {
            result = tool_registry.execute(call.name, call.args)?;
            messages.push(tool_result(call.id, result));
        }
    } else {
        emit("chat_chunk", response.content);
        break;
    }
}
```

**Rust 文件位置**: `src-tauri/src/agent/`

---

### Task 4: MCP 协议客户端

**目标**: 实现 MCP Client，能连接外部 MCP Server，获取其 tools/resources。

**必须参考的文件**:
```
ref/astrbot/astrbot/core/agent/mcp_client.py                    (16KB, 完整 MCP 客户端实现!)
ref/openclaw/packages/                                            (MCP 相关包)
ref/openclaw/src/                                                 (核心架构)
```

**MCP 规范**: https://modelcontextprotocol.io/

**实现要点**:
1. stdio transport: spawn 子进程，通过 stdin/stdout JSON-RPC 通信
2. SSE transport: HTTP 长连接 (可选)
3. 实现 `initialize` / `tools/list` / `tools/call` / `resources/list` / `resources/read` 方法
4. 将 MCP Server 的工具注册到全局 ToolSet

**Rust 文件位置**: `src-tauri/src/mcp/`

---

### Task 5: Skill 技能系统

**目标**: 解析 SKILL.md 文件，加载技能定义，注入到 Agent。

**必须参考的文件**:
```
ref/astrbot/astrbot/core/skills/skill_manager.py                (27KB, 技能管理器)
ref/astrbot/astrbot/core/skills/neo_skill_sync.py               (13KB, 技能同步)
ref/claurst/README.md                                            (Line 106-166, BUDDY 系统 = Skill 变种)
```

**Skill 定义格式**:
```yaml
---
name: code-review
description: 全面的代码质量审查
---
[具体的 system prompt instructions]
```

**Rust 文件位置**: `src-tauri/src/skill/`

---

### Task 6: RAG 知识检索

**目标**: 对代码仓库建立向量索引，支持语义搜索。

**必须参考的文件**:
```
ref/astrbot/astrbot/core/knowledge_base/                         (知识库完整实现)
ref/astrbot/astrbot/core/provider/sources/openai_embedding_source.py (Embedding 调用)
ref/astrbot/astrbot/core/provider/sources/gemini_embedding_source.py
```

**实现要点**:
1. 代码切分: 按函数/类边界切分 (用 tree-sitter 或简单行切分)
2. Embedding: 调用 OpenAI/其他 Embedding API
3. 存储: SQLite + 余弦相似度排序 (简单方案)
4. 检索 API: `invoke("knowledge_search", { query, topK })`

**Rust 文件位置**: `src-tauri/src/knowledge/`

---

### Task 7: Harness 执行框架

**目标**: Agent 运行时的安全壳 — 权限、Token 预算、沙箱。

**必须参考的文件**:
```
ref/claurst/README.md                                            (Line 400-414, 权限系统)
ref/oh-my-openagent/README.md                                    (Line 213-231, Hash-Anchored Editing)
```

**Rust 文件位置**: `src-tauri/src/harness/`

---

## 3. 前端 IPC 接口约定

前端通过 `@tauri-apps/api/core` 的 `invoke()` 调用后端。以下是前端期望的 command 列表：

```rust
// ===== 聊天 =====
#[tauri::command] async fn chat_send(messages: Vec<Message>, provider_id: String) -> Result<()>
// 流式响应通过 app.emit("chat_chunk", payload) 推送

// ===== Provider =====
#[tauri::command] async fn provider_list() -> Result<Vec<Provider>>
#[tauri::command] async fn provider_create(config: ProviderConfig) -> Result<Provider>
#[tauri::command] async fn provider_delete(id: String) -> Result<()>

// ===== Agent =====
#[tauri::command] async fn agent_list() -> Result<Vec<Agent>>
#[tauri::command] async fn agent_create(config: AgentConfig) -> Result<Agent>
#[tauri::command] async fn agent_start(id: String) -> Result<()>
#[tauri::command] async fn agent_stop(id: String) -> Result<()>

// ===== Tool =====
#[tauri::command] async fn tool_list() -> Result<Vec<ToolSchema>>
#[tauri::command] async fn tool_execute(name: String, args: serde_json::Value) -> Result<String>

// ===== MCP =====
#[tauri::command] async fn mcp_server_list() -> Result<Vec<McpServer>>
#[tauri::command] async fn mcp_server_add(config: McpServerConfig) -> Result<McpServer>
#[tauri::command] async fn mcp_server_remove(id: String) -> Result<()>
#[tauri::command] async fn mcp_server_tools(id: String) -> Result<Vec<ToolSchema>>

// ===== Skill =====
#[tauri::command] async fn skill_list() -> Result<Vec<Skill>>
#[tauri::command] async fn skill_toggle(name: String, enabled: bool) -> Result<()>

// ===== Knowledge =====
#[tauri::command] async fn knowledge_repos() -> Result<Vec<KnowledgeRepo>>
#[tauri::command] async fn knowledge_index(path: String) -> Result<()>
#[tauri::command] async fn knowledge_search(query: String, top_k: usize) -> Result<Vec<SearchResult>>

// ===== Logs =====
#[tauri::command] async fn log_list(limit: usize) -> Result<Vec<TraceLog>>

// ===== Settings =====
#[tauri::command] async fn settings_get() -> Result<AppSettings>
#[tauri::command] async fn settings_update(settings: AppSettings) -> Result<()>

// ===== Project =====
#[tauri::command] async fn project_open(path: String) -> Result<ProjectInfo>
#[tauri::command] async fn project_review(path: String) -> Result<Vec<ReviewIssue>>
```

## 4. Cargo 依赖

```toml
[dependencies]
tauri = { version = "2", features = [] }
tauri-plugin-opener = "2"
tauri-plugin-fs = "2"
tauri-plugin-shell = "2"
tauri-plugin-dialog = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", features = ["json", "stream"] }
futures = "0.3"
async-trait = "0.1"
uuid = { version = "1", features = ["v4"] }
chrono = { version = "0.4", features = ["serde"] }
rusqlite = { version = "0.31", features = ["bundled"] }
walkdir = "2"
regex = "1"
```

## 5. 开发顺序

**必须按此顺序，每完成一个 Task 确保前端对应页面能用**:

1. **Task 1: LLM Provider** → 前端 `/providers` 和 `/chat` 页面能用
2. **Task 2: Tool Calling** → 前端 `/tools` 页面能用
3. **Task 3: Agent Loop** → 前端 `/chat` 完整可用（聊天 + 工具调用）
4. **Task 4: MCP Client** → 前端 `/mcp` 页面能用
5. **Task 5: Skill System** → 前端 `/skills` 页面能用
6. **Task 6: RAG** → 前端 `/knowledge` 页面能用
7. **Task 7: Harness** → 前端 `/settings` 安全配置生效

## 6. ⚠️ 再次强调

- **每个 Task 开始前**，先读完参考文件再写代码
- **Agent 相关逻辑**绝不能凭空编写，`ref/astrbot/astrbot/core/` 是最关键的参考
- **MCP 协议**必须按规范实现，`ref/astrbot/astrbot/core/agent/mcp_client.py` 是唯一可信参考
- **工具格式**必须兼容 OpenAI Function Calling，参考 `ref/astrbot/astrbot/core/agent/tool.py` 的 `openai_schema()` 方法
