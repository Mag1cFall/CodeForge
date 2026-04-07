# CodeForge Rust 模块参考映射

本文档整理 `src-tauri/src/` 九个模块与 `ref/` 下五个参考项目的对应实现入口，并标记当前已补齐与仍待继续收敛的差距。

## 1. llm

- 当前模块：`src-tauri/src/llm/{openai_compatible.rs,anthropic.rs,streaming.rs,store.rs,model.rs,provider.rs}`
- astrbot：
  - `ref/astrbot/astrbot/core/provider/sources/openai_source.py`
  - `ref/astrbot/astrbot/core/provider/sources/anthropic_source.py`
  - `ref/astrbot/astrbot/core/provider/sources/openai_embedding_source.py`
- openclaw：
  - `ref/openclaw/src/agents/context.ts`
  - `ref/openclaw/docs/concepts/model-providers.md`
  - `ref/openclaw/src/agents/chutes-models.ts`
- opencode：
  - `ref/opencode/packages/opencode/test/provider/copilot/copilot-chat-model.test.ts`
  - `ref/opencode/packages/opencode/test/session/prompt.test.ts`
- claurst：
  - `ref/claurst/src-rust/crates/api/src/providers/*.rs`
  - `ref/claurst/src-rust/crates/api/src/model_registry.rs`
- oh-my-openagent：
  - `ref/oh-my-openagent/src/agents/sisyphus/gpt-5-4.ts`
  - `ref/oh-my-openagent/src/cli/run/model-resolver.test.ts`
- 当前状态：
  - 已补齐 OpenAI/Anthropic SSE 解析、模型上下文窗口动态探测优先级、Provider 默认初始化。
  - 仍缺少更完整的 provider 重试策略、模型能力元数据缓存与 provider-specific capability 归一化。

## 2. tools

- 当前模块：`src-tauri/src/tools/{registry.rs,file_tools.rs,search_tools.rs,analysis_tools.rs,shell_tools.rs,schema.rs}`
- astrbot：
  - `ref/astrbot/astrbot/core/agent/tool.py`
  - `ref/astrbot/astrbot/core/agent/tool_executor.py`
  - `ref/astrbot/astrbot/core/computer/tools/{fs.py,shell.py,permissions.py}`
- openclaw：
  - `ref/openclaw/docs/tools/*.md`
  - `ref/openclaw/ui/src/ui/chat/tool-helpers.ts`
- opencode：
  - `ref/opencode/packages/ui/src/components/tool-*.tsx`
- claurst：
  - `ref/claurst/src-rust/crates/tools/src/{bash.rs,powershell.rs,file_read.rs,file_write.rs,apply_patch.rs,skill_tool.rs}`
  - `ref/claurst/spec/03_tools.md`
- oh-my-openagent：
  - `ref/oh-my-openagent/src/tools/{skill,lsp,look-at,session-manager}/**/*`
- 当前状态：
  - 已补齐 OpenAI/Anthropic schema 转换、shell 沙箱目录约束、Windows 常见 shell alias 兼容。
  - 仍缺少更严格的路径越界保护、批量补丁能力、工具结果分段与大输出分页。

## 3. agent

- 当前模块：`src-tauri/src/agent/{runner.rs,context.rs,hooks.rs,prompt.rs,definition.rs,orchestrator.rs}`
- astrbot：
  - `ref/astrbot/astrbot/core/agent/context/{manager.py,compressor.py,token_counter.py,truncator.py}`
  - `ref/astrbot/astrbot/core/agent/hooks.py`
  - `ref/astrbot/astrbot/core/agent/agent.py`
  - `ref/astrbot/astrbot/core/agent/run_context.py`
- openclaw：
  - `ref/openclaw/src/agents/**/*.ts`
  - `ref/openclaw/src/context-engine/**/*.ts`
- opencode：
  - `ref/opencode/packages/app/src/pages/session/**/*.tsx`
  - `ref/opencode/packages/app/src/context/prompt.tsx`
- claurst：
  - `ref/claurst/spec/{05_components_agents_permissions_design.md,06_services_context_state.md,07_hooks.md}`
  - `ref/claurst/src-rust/crates/query/src/{coordinator.rs,context_analyzer.rs,compact.rs}`
- oh-my-openagent：
  - `ref/oh-my-openagent/src/agents/**/*.ts`
  - `ref/oh-my-openagent/src/plugin/{event.ts,tool-execute-before.ts}`
- 当前状态：
  - 已补齐上下文压缩、hook trace、权限批准后续跑、live permission resume 回归测试。
  - 仍缺少多轮 tool loop 的更细粒度事件流、会话级模型覆盖、agent 运行策略配置化。

## 4. mcp

- 当前模块：`src-tauri/src/mcp/{client.rs,server_mgr.rs,transport.rs}`
- astrbot：
  - `ref/astrbot/astrbot/core/agent/mcp_client.py`
- openclaw：
  - `ref/openclaw/docs/tools/skills.md`
  - `ref/openclaw/src/plugins/**/*.ts`
- opencode：
  - 以 session / skill / tool 侧集成为主，没有单独 Rust MCP 客户端。
- claurst：
  - `ref/claurst/src-rust/crates/mcp/src/{lib.rs,registry.rs,oauth.rs}`
  - `ref/claurst/src-rust/crates/tools/src/{mcp_resources.rs,mcp_auth_tool.rs}`
- oh-my-openagent：
  - `ref/oh-my-openagent/src/tools/skill-mcp/**/*`
- 当前状态：
  - 已打通 initialize → tools/list → tools/call → resources/list → resources/read。
  - 仍缺少长连接复用、认证刷新、失败重连与资源缓存。

## 5. skill

- 当前模块：`src-tauri/src/skill/{loader.rs,manager.rs}`
- astrbot：
  - `ref/astrbot/astrbot/core/skills/{skill_manager.py,neo_skill_sync.py}`
- openclaw：
  - `ref/openclaw/ui/src/ui/views/skills.ts`
  - `ref/openclaw/src/config/types.skills.ts`
- opencode：
  - `ref/opencode/packages/opencode/test/skill/discovery.test.ts`
- claurst：
  - `ref/claurst/src-rust/crates/core/src/skill_discovery.rs`
  - `ref/claurst/src-rust/crates/query/src/skill_prefetch.rs`
  - `ref/claurst/src-rust/crates/tools/src/bundled_skills.rs`
- oh-my-openagent：
  - `ref/oh-my-openagent/src/tools/skill/**/*`
  - `ref/oh-my-openagent/src/features/builtin-skills/**/*`
- 当前状态：
  - 已改成内置 skill 默认启用、用户 skill 默认关闭、上下文只注入目录清单不注入全文。
  - 仍缺少按任务自动挑选 skill、异步描述刷新、优先级冲突解决。

## 6. knowledge

- 当前模块：`src-tauri/src/knowledge/{embedder.rs,indexer.rs,retriever.rs,store.rs}`
- astrbot：
  - `ref/astrbot/astrbot/core/provider/sources/openai_embedding_source.py`
  - 其余以知识库/管理测试和 provider 侧嵌入为主。
- openclaw：
  - `ref/openclaw/docs/concepts/context.md`
  - `ref/openclaw/docs/concepts/session-pruning.md`
  - `ref/openclaw/docs/concepts/model-providers.md` 中 embedding provider hook
- opencode：
  - 间接对应 session/context/worker 侧上下文消费，没有独立知识库模块。
- claurst：
  - `ref/claurst/src-rust/crates/query/src/session_memory.rs`
  - `ref/claurst/src-rust/crates/query/src/context_analyzer.rs`
- oh-my-openagent：
  - 以 skill / session 存储和工具发现为主，没有独立 RAG 模块。
- 当前状态：
  - 已接真实 embedding API、异步安全请求、HTTP 错误重试、快速夹具知识库烟测。
  - 仍缺少基于 token 的 chunk 策略、批量 embedding、向量缓存与增量索引。

## 7. harness

- 当前模块：`src-tauri/src/harness/{budget.rs,compression.rs,permission.rs,sandbox.rs,hashline.rs}`
- astrbot：
  - `ref/astrbot/docs/zh/use/astrbot-agent-sandbox.md`
  - `ref/astrbot/astrbot/core/computer/tools/permissions.py`
- openclaw：
  - `ref/openclaw/docs/tools/exec-approvals.md`
  - `ref/openclaw/docs/tools/multi-agent-sandbox-tools.md`
  - `ref/openclaw/src/config/types.sandbox.ts`
- opencode：
  - `ref/opencode/packages/opencode/test/permission-task.test.ts`
- claurst：
  - `ref/claurst/spec/{05_components_agents_permissions_design.md,06_services_context_state.md}`
  - `ref/claurst/src-rust/crates/core/src/bash_classifier.rs`
- oh-my-openagent：
  - `ref/oh-my-openagent/src/plugin/tool-execute-before.ts`
  - `ref/oh-my-openagent/src/tools/task/**/*`
- 当前状态：
  - 已支持 AskUser 权限、上下文压缩、shell 沙箱隔离与 hashline。
  - 仍缺少持久化审批恢复、审批队列 UI/服务分层、超时后自动续跑策略。

## 8. session

- 当前模块：`src-tauri/src/session/{manager.rs,message_mutations.rs,persistence.rs,mod.rs}`
- astrbot：
  - `ref/astrbot/tests/unit/test_session_lock.py`
  - `ref/astrbot/astrbot/core/conversation_mgr.py`
- openclaw：
  - `ref/openclaw/src/auto-reply/reply/session*.ts`
  - `ref/openclaw/docs/reference/session-management-compaction.md`
- opencode：
  - `ref/opencode/packages/app/src/pages/session.tsx`
  - `ref/opencode/packages/opencode/test/session/*.test.ts`
- claurst：
  - `ref/claurst/src-rust/crates/core/src/session_storage.rs`
  - `ref/claurst/src-rust/crates/query/src/session_memory.rs`
- oh-my-openagent：
  - `ref/oh-my-openagent/src/tools/session-manager/**/*`
- 当前状态：
  - 已支持消息重写、权限续跑、上下文窗口覆盖、当前上下文 token 估算。
  - 仍缺少分支会话、恢复点、消息级 metadata 更细拆分。

## 9. logging

- 当前模块：`src-tauri/src/logging/{service.rs,mod.rs}`
- astrbot：
  - 以 agent hook / provider / dashboard 路由日志与 changelog 回归为主。
- openclaw：
  - `ref/openclaw/src/daemon/service-audit.ts`
  - `ref/openclaw/src/tui/notifications.rs` 等事件/状态日志面
- opencode：
  - `ref/opencode/packages/app/src/utils/server-errors.ts`
  - session 页面状态与 toast 流程
- claurst：
  - `ref/claurst/spec/07_hooks.md`
  - `ref/claurst/src-rust/crates/tui/src/notifications.rs`
- oh-my-openagent：
  - `ref/oh-my-openagent/src/cli/run/{event-handlers.ts,event-formatting.ts}`
  - `ref/oh-my-openagent/src/tools/background-task/task-status-format.ts`
- 当前状态：
  - 已记录 chat/tool/review/provider/permission/hook 事件。
  - 仍缺少结构化等级、关联 trace id、前端订阅式实时日志流。

## 当前通过的自动化链路

- `cargo test`: 34 passed / 4 ignored
- `npm run build`: passed
- `cargo run --bin manual_qa`: passed
- `cargo test live_runner_permission_resume -- --ignored --nocapture`: passed

## 当前已经确认修复的关键断点

1. 异步上下文中误用 blocking reqwest，导致 tokio runtime panic。
2. skill 全文注入 system prompt，导致上下文爆炸。
3. 权限弹窗批准后只记账，不继续执行 tool/LLM。
4. Windows 下 shell 命令兼容性不足，`pwd` 导致续跑失败。
5. manual QA 过重且不可持续，已改为夹具仓库烟测。
