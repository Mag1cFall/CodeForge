import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

export interface AgentRecord {
  id: string;
  name: string;
  instructions?: string | null;
  status: 'idle' | 'running' | 'stopped';
  model: string;
  tools: string[];
  createdAt: string;
  updatedAt: string;
}

export interface AgentConfigInput {
  name: string;
  instructions?: string | null;
  tools: string[];
  model: string;
}

export type ProviderType = 'openAiCompatible' | 'anthropic';

export interface ProviderSummary {
  id: string;
  name: string;
  providerType: ProviderType;
  endpoint: string;
  apiKey?: string | null;
  model: string;
  models: string[];
  enabled: boolean;
  keySet: boolean;
  isDefault: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface ProviderConfigInput {
  name: string;
  providerType: ProviderType;
  endpoint: string;
  apiKey?: string | null;
  model: string;
  models: string[];
  enabled: boolean;
  isDefault: boolean;
  headers: Record<string, string>;
}

export interface SessionRecord {
  id: string;
  title: string;
  agentId: string;
  contextTokensUsed: number;
  contextTokensMax: number;
  createdAt: string;
  updatedAt: string;
}

export interface SessionMessage {
  id: string;
  sessionId: string;
  role: 'user' | 'assistant' | 'tool';
  content: string;
  toolCalls: Array<Record<string, unknown>>;
  createdAt: string;
}

export interface ToolSchema {
  name: string;
  description: string;
  parameters: Record<string, unknown>;
}

export interface McpServerRecord {
  id: string;
  name: string;
  transport: string;
  command?: string | null;
  url?: string | null;
  args: string[];
  env: Record<string, string>;
  headers: Record<string, string>;
  enabled: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface McpServerConfigInput {
  name: string;
  transport: string;
  command?: string | null;
  url?: string | null;
  args: string[];
  env: Record<string, string>;
  headers: Record<string, string>;
  enabled: boolean;
}

export interface McpToolInfo {
  name: string;
  description: string;
  inputSchema: Record<string, unknown>;
}

export interface SkillRecord {
  id: string;
  name: string;
  description: string;
  path: string;
  instructions: string;
  tools: string[];
  mcpServers: string[];
  enabled: boolean;
}

export interface KnowledgeRepo {
  id: string;
  path: string;
  status: string;
  chunkCount: number;
  updatedAt: string;
}

export interface SearchResult {
  filePath: string;
  content: string;
  score: number;
}

export interface TraceLog {
  id: number;
  kind: string;
  payload: Record<string, unknown>;
  createdAt: string;
}

export interface AppSettings {
  theme: string;
  language: string;
  projectPath?: string | null;
  skillsPath?: string | null;
  contextWindowOverrides?: Record<string, number>;
}

export interface EmbeddingConfig {
  endpoint: string;
  model: string;
  apiKey?: string | null;
}

export interface SessionRunConfig {
  providerId?: string | null;
  model?: string | null;
  stream?: boolean;
}

export interface ProjectInfo {
  path: string;
  fileCount: number;
  name: string;
}

export interface ChatChunkEvent {
  sessionId: string;
  delta: string;
  done: boolean;
  toolResults: Array<Record<string, unknown>>;
}

export interface ChatProgressEvent {
  sessionId: string;
  message: string;
}

export interface ChatToolResultEvent {
  sessionId: string;
  tool: Record<string, unknown>;
}

export interface PermissionRequestPayload {
  id: string;
  toolName: string;
  args: Record<string, unknown>;
  riskLevel: 'low' | 'medium' | 'high';
  description: string;
}

export interface ReviewProgressEvent {
  step: string;
  log: string;
}

export interface ReviewIssue {
  file: string;
  line: number;
  rule: string;
  severity: string;
  message: string;
  suggestion: string;
}

export interface ToolUsageCount {
  name: string;
  calls: number;
}

export const agentList = () => invoke<AgentRecord[]>('agent_list');
export const agentCreate = (config: AgentConfigInput) => invoke<AgentRecord>('agent_create', { config });
export const agentStart = (id: string) => invoke<void>('agent_start', { id });
export const agentStop = (id: string) => invoke<void>('agent_stop', { id });

export const providerList = () => invoke<ProviderSummary[]>('provider_list');
export const providerCreate = (config: ProviderConfigInput) => invoke<ProviderSummary>('provider_create', { config });
export const providerUpdate = (id: string, config: ProviderConfigInput) => invoke<ProviderSummary>('provider_update', { id, config });
export const providerDelete = (id: string) => invoke<void>('provider_delete', { id });
export const providerFetchModels = (providerType: ProviderType, endpoint: string, apiKey?: string | null, headers?: Record<string, string>) =>
  invoke<string[]>('provider_fetch_models', { providerType, endpoint, apiKey, headers });

export const sessionList = () => invoke<SessionRecord[]>('session_list');
export const sessionCreate = (agentId: string) => invoke<SessionRecord>('session_create', { agentId });
export const sessionDelete = (id: string) => invoke<void>('session_delete', { id });
export const sessionMessages = (id: string) => invoke<SessionMessage[]>('session_messages', { id });
export const sessionRewriteMessage = (sessionId: string, messageId: string, content: string) => invoke<void>('session_rewrite_message', { sessionId, messageId, content });

export const chatSend = (sessionId: string, message: string, config?: SessionRunConfig) => invoke<void>('chat_send', { sessionId, message, config });
export const chatRetry = (sessionId: string, messageId?: string | null) => invoke<void>('chat_retry', { sessionId, messageId });
export const permissionRespond = (requestId: string, approved: boolean) => invoke<void>('permission_respond', { requestId, approved });
export const permissionPending = (sessionId: string) => invoke<PermissionRequestPayload | null>('permission_pending', { sessionId });

export const toolList = () => invoke<ToolSchema[]>('tool_list');
export const toolExecute = (name: string, args: Record<string, unknown>) => invoke<string>('tool_execute', { name, args });
export const toolUsageCounts = () => invoke<ToolUsageCount[]>('tool_usage_counts');

export const mcpServerList = () => invoke<McpServerRecord[]>('mcp_server_list');
export const mcpServerAdd = (config: McpServerConfigInput) => invoke<McpServerRecord>('mcp_server_add', { config });
export const mcpServerRemove = (id: string) => invoke<void>('mcp_server_remove', { id });
export const mcpServerTools = (id: string) => invoke<McpToolInfo[]>('mcp_server_tools', { id });

export const skillList = () => invoke<SkillRecord[]>('skill_list');
export const skillToggle = (name: string, enabled: boolean) => invoke<void>('skill_toggle', { name, enabled });

export const knowledgeRepos = () => invoke<KnowledgeRepo[]>('knowledge_repos');
export const knowledgeIndex = (path: string) => invoke<void>('knowledge_index', { path });
export const knowledgeSearch = (query: string, topK: number) => invoke<SearchResult[]>('knowledge_search', { query, topK });

export const logList = (limit: number) => invoke<TraceLog[]>('log_list', { limit });

export const settingsGet = () => invoke<AppSettings>('settings_get');
export const settingsUpdate = (settings: AppSettings) => invoke<void>('settings_update', { settings });
export const embeddingConfigGet = () => invoke<EmbeddingConfig>('embedding_config_get');

export const projectOpen = (path: string) => invoke<ProjectInfo>('project_open', { path });
export const projectClone = (gitUrl: string) => invoke<ProjectInfo>('project_clone', { gitUrl });
export const projectReview = (path: string, sandbox: boolean) => invoke<void>('project_review', { path, sandbox });

export const listenChatChunk = (handler: (payload: ChatChunkEvent) => void): Promise<UnlistenFn> =>
  listen<ChatChunkEvent>('chat_chunk', (event) => {
    handler(event.payload);
  });

export const listenChatProgress = (handler: (payload: ChatProgressEvent) => void): Promise<UnlistenFn> =>
  listen<ChatProgressEvent>('chat_progress', (event) => {
    handler(event.payload);
  });

export const listenChatToolResult = (handler: (payload: ChatToolResultEvent) => void): Promise<UnlistenFn> =>
  listen<ChatToolResultEvent>('chat_tool_result', (event) => {
    handler(event.payload);
  });

export const listenPermissionRequest = (handler: (payload: PermissionRequestPayload) => void): Promise<UnlistenFn> =>
  listen<PermissionRequestPayload>('permission_request', (event) => {
    handler(event.payload);
  });

export const listenReviewProgress = (handler: (payload: ReviewProgressEvent) => void): Promise<UnlistenFn> =>
  listen<ReviewProgressEvent>('review_progress', (event) => {
    handler(event.payload);
  });

export const listenReviewResult = (handler: (payload: ReviewIssue[]) => void): Promise<UnlistenFn> =>
  listen<ReviewIssue[]>('review_result', (event) => {
    handler(event.payload);
  });
