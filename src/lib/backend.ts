import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export type ProviderType = 'openAiCompatible' | 'anthropic';

export interface ProviderSummary {
  id: string;
  name: string;
  providerType: ProviderType;
  endpoint: string;
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

export interface AgentRecord {
  id: string;
  name: string;
  instructions?: string | null;
  tools: string[];
  model: string;
  status: 'idle' | 'running' | 'stopped';
  createdAt: string;
  updatedAt: string;
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

export interface PermissionRequest {
  id: string;
  toolName: string;
  args: Record<string, unknown>;
  riskLevel: 'low' | 'medium' | 'high';
  description: string;
}

export interface ChatChunkEvent {
  sessionId: string;
  delta: string;
  done: boolean;
  toolResults: Array<Record<string, unknown>>;
}

export interface ReviewIssue {
  file: string;
  line: number;
  rule: string;
  severity: 'error' | 'warning' | 'info';
  message: string;
  suggestion: string;
}

export interface ReviewProgressEvent {
  step: string;
  log: string;
}

export interface AppSettings {
  theme: string;
  language: string;
  projectPath?: string | null;
}

export const providerList = () => invoke<ProviderSummary[]>('provider_list');
export const providerCreate = (config: ProviderConfigInput) => invoke<ProviderSummary>('provider_create', { config });
export const providerDelete = (id: string) => invoke<void>('provider_delete', { id });

export const agentList = () => invoke<AgentRecord[]>('agent_list');

export const sessionList = () => invoke<SessionRecord[]>('session_list');
export const sessionCreate = (agentId: string) => invoke<SessionRecord>('session_create', { agentId });
export const sessionMessages = (id: string) => invoke<SessionMessage[]>('session_messages', { id });

export const chatSend = (sessionId: string, message: string) => invoke<void>('chat_send', { sessionId, message });
export const permissionRespond = (requestId: string, approved: boolean) => invoke<void>('permission_respond', { requestId, approved });

export const settingsGet = () => invoke<AppSettings>('settings_get');
export const settingsUpdate = (settings: AppSettings) => invoke<void>('settings_update', { settings });

export const projectReview = (path: string, sandbox: boolean) => invoke<void>('project_review', { path, sandbox });
export const projectClone = (gitUrl: string) => invoke<{ path: string; fileCount: number; name: string }>('project_clone', { gitUrl });

export const listenChatChunk = (handler: (payload: ChatChunkEvent) => void) => listen<ChatChunkEvent>('chat_chunk', (event) => handler(event.payload));
export const listenPermissionRequest = (handler: (payload: PermissionRequest) => void) => listen<PermissionRequest>('permission_request', (event) => handler(event.payload));
export const listenReviewProgress = (handler: (payload: ReviewProgressEvent) => void) => listen<ReviewProgressEvent>('review_progress', (event) => handler(event.payload));
export const listenReviewResult = (handler: (payload: ReviewIssue[]) => void) => listen<ReviewIssue[]>('review_result', (event) => handler(event.payload));
