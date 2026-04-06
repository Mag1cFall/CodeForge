import { useState, useRef, useEffect, useCallback } from 'react';
import {
  Send, Bot, User, Loader2, Paperclip, Sparkles, RotateCcw, Copy, Wrench, Pencil,
  PanelRightOpen, PanelRightClose, Plus, MessageSquare, Gauge, Layers,
  Settings2, Thermometer
} from 'lucide-react';
import PermissionDialog, { PermissionRequest } from '../components/PermissionDialog';
import {
  agentList,
  embeddingConfigGet,
  chatSend,
  chatRetry,
  listenChatChunk,
  listenPermissionRequest,
  permissionRespond,
  providerList,
  ProviderSummary,
  sessionRewriteMessage,
  sessionCreate,
  sessionList,
  sessionMessages,
  SessionMessage,
} from '../lib/backend';
import './Chat.css';

interface ChatMessage {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  timestamp: Date;
  toolCalls?: { name: string; args: string; result?: string }[];
}

interface Session {
  id: string;
  title: string;
  messageCount: number;
  tokenUsed: number;
  tokenMax: number;
  createdAt: Date;
}

const isRecord = (value: unknown): value is Record<string, unknown> => {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
};

const readString = (value: Record<string, unknown>, key: string): string => {
  const target = value[key];
  return typeof target === 'string' ? target : '';
};

const toToolCalls = (items: Array<Record<string, unknown>>): NonNullable<ChatMessage['toolCalls']> => {
  return items.map((item) => {
    const result = item.result;
    return {
      name: readString(item, 'name') || 'tool',
      args: JSON.stringify(item, null, 2),
      result: typeof result === 'string' ? result : JSON.stringify(result, null, 2),
    };
  });
};

const parseTimestamp = (value: string): Date => {
  const parsed = new Date(value);
  return Number.isNaN(parsed.getTime()) ? new Date() : parsed;
};

const toProviderModels = (provider?: ProviderSummary): string[] => {
  if (!provider) {
    return [];
  }
  const models = provider.models.length > 0 ? provider.models : [provider.model];
  return models.filter((model) => model.length > 0);
};

const getDefaultProviderId = (providers: ProviderSummary[]): string => {
  return providers.find((item) => item.isDefault)?.id || providers[0]?.id || '';
};

const EMBEDDING_ENV_PROVIDER_ID = '__embedding_env__';

const toChatMessage = (message: SessionMessage): ChatMessage => {
  const role: ChatMessage['role'] = message.role === 'user' ? 'user' : 'assistant';
  const toolCalls = toToolCalls(message.toolCalls.filter(isRecord));

  return {
    id: message.id,
    role,
    content: message.content,
    timestamp: parseTimestamp(message.createdAt),
    toolCalls: toolCalls.length > 0 ? toolCalls : undefined,
  };
};

export default function Chat() {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [sessions, setSessions] = useState<Session[]>([]);
  const [providerOptions, setProviderOptions] = useState<ProviderSummary[]>([]);
  const [input, setInput] = useState('');
  const [editingMessageId, setEditingMessageId] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [showPanel, setShowPanel] = useState(true);
  const [activeSession, setActiveSession] = useState('');
  const [permRequest, setPermRequest] = useState<PermissionRequest | null>(null);
  const [provider, setProvider] = useState('');
  const [model, setModel] = useState('');
  const [temperature, setTemperature] = useState(0.7);
  const [topP, setTopP] = useState(0.9);
  const [maxTokens, setMaxTokens] = useState(64000);
  const [streaming, setStreaming] = useState(true);
  const [embeddingProvider, setEmbeddingProvider] = useState('');
  const [embeddingEndpoint, setEmbeddingEndpoint] = useState('');
  const [embeddingModel, setEmbeddingModel] = useState('');
  const [embeddingApiKey, setEmbeddingApiKey] = useState('');
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const activeSessionRef = useRef('');
  const pendingAssistantIdRef = useRef<string | null>(null);

  const selectedProvider = providerOptions.find((item) => item.id === provider);
  const modelOptions = toProviderModels(selectedProvider);
  const selectedEmbeddingProvider = providerOptions.find((item) => item.id === embeddingProvider);
  const embeddingModelOptions = Array.from(new Set([
    ...toProviderModels(selectedEmbeddingProvider),
    ...(embeddingModel ? [embeddingModel] : []),
  ])).filter((item) => item.length > 0);

  const activeSessionData = sessions.find((item) => item.id === activeSession);
  const contextUsed = activeSessionData?.tokenUsed ?? 0;
  const contextMax = activeSessionData?.tokenMax ?? 1_000_000;
  const contextPercent = contextMax > 0 ? Math.round((contextUsed / contextMax) * 100) : 0;

  useEffect(() => {
    activeSessionRef.current = activeSession;
    setEditingMessageId(null);
  }, [activeSession]);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);

  const applyEmbeddingProvider = useCallback((providerId: string, providers: ProviderSummary[]) => {
    if (providerId === EMBEDDING_ENV_PROVIDER_ID) {
      setEmbeddingProvider(providerId);
      return;
    }
    const selected = providers.find((item) => item.id === providerId);
    setEmbeddingProvider(providerId);
    if (!selected) {
      return;
    }

    const models = toProviderModels(selected);
    setEmbeddingEndpoint(selected.endpoint);
    setEmbeddingApiKey(selected.apiKey ?? '');
    setEmbeddingModel(selected.model || models[0] || '');
  }, []);

  const loadEmbeddingConfig = useCallback(async (providers: ProviderSummary[]) => {
    try {
      const config = await embeddingConfigGet();
      const matchedProvider = providers.find((item) => {
        const models = toProviderModels(item);
        return item.endpoint === config.endpoint && (models.includes(config.model) || item.model === config.model);
      });

      setEmbeddingProvider(matchedProvider?.id || EMBEDDING_ENV_PROVIDER_ID);
      setEmbeddingEndpoint(config.endpoint || matchedProvider?.endpoint || '');
      setEmbeddingModel(config.model || matchedProvider?.model || '');
      setEmbeddingApiKey(config.apiKey || matchedProvider?.apiKey || '');
    } catch {
      const defaultProviderId = getDefaultProviderId(providers);
      if (!defaultProviderId) {
        setEmbeddingProvider('');
        setEmbeddingEndpoint('');
        setEmbeddingModel('');
        setEmbeddingApiKey('');
        return;
      }
      applyEmbeddingProvider(defaultProviderId, providers);
    }
  }, [applyEmbeddingProvider]);

  const loadSessionMessages = useCallback(async (sessionId: string) => {
    if (!sessionId) {
      setMessages([]);
      return;
    }

    try {
      const data = await sessionMessages(sessionId);
      setMessages((data ?? []).map(toChatMessage));
    } catch {
      setMessages([]);
    }
  }, []);

  const loadSessions = useCallback(async () => {
    try {
      const records = await sessionList();
      const next = await Promise.all(
        (records ?? []).map(async (record) => {
          let messageCount = 0;
          try {
            messageCount = (await sessionMessages(record.id)).length;
          } catch {
            messageCount = 0;
          }

          return {
            id: record.id,
            title: record.title,
            messageCount,
            tokenUsed: record.contextTokensUsed,
            tokenMax: record.contextTokensMax,
            createdAt: parseTimestamp(record.createdAt),
          };
        })
      );

      setSessions(next);
      if (next.length === 0) {
        setActiveSession('');
        return;
      }
      setActiveSession((prev) => (next.some((item) => item.id === prev) ? prev : next[0].id));
    } catch {
      setSessions([]);
      setActiveSession('');
    }
  }, []);

  useEffect(() => {
    void (async () => {
      try {
        const data = await providerList();
        const list = (data ?? []).filter((item) => item.enabled);
        const next = list.length > 0 ? list : (data ?? []);
        const defaultProviderId = getDefaultProviderId(next);

        setProviderOptions(next);
        setProvider((prev) => (next.some((item) => item.id === prev) ? prev : defaultProviderId));

        if (next.length === 0) {
          setEmbeddingProvider('');
          setEmbeddingEndpoint('');
          setEmbeddingModel('');
          setEmbeddingApiKey('');
        } else {
          await loadEmbeddingConfig(next);
        }
      } catch {
        setProviderOptions([]);
        setProvider('');
        setEmbeddingProvider('');
        setEmbeddingEndpoint('');
        setEmbeddingModel('');
        setEmbeddingApiKey('');
      }
    })();
    void loadSessions();
  }, [loadEmbeddingConfig, loadSessions]);

  useEffect(() => {
    if (!selectedProvider) {
      setModel('');
      return;
    }

    const models = toProviderModels(selectedProvider);
    if (models.length === 0) {
      setModel('');
      return;
    }

    if (!models.includes(model)) {
      setModel(selectedProvider.model || models[0]);
    }
  }, [selectedProvider, model]);

  useEffect(() => {
    if (!activeSession) {
      setMessages([]);
      return;
    }
    void loadSessionMessages(activeSession);
  }, [activeSession, loadSessionMessages]);

  useEffect(() => {
    let unlisten: (() => void) | null = null;

    const subscribe = async () => {
      try {
        unlisten = await listenChatChunk((payload) => {
          if (payload.sessionId !== activeSessionRef.current) {
            return;
          }

          const pendingId = pendingAssistantIdRef.current;
          if (pendingId) {
            setMessages((prev) => prev.map((item) => {
              if (item.id !== pendingId) {
                return item;
              }

              return {
                ...item,
                content: `${item.content}${payload.delta}`,
                toolCalls: payload.done && payload.toolResults.length > 0
                  ? toToolCalls(payload.toolResults.filter(isRecord))
                  : item.toolCalls,
              };
            }));
          } else if (payload.delta || payload.toolResults.length > 0) {
            setMessages((prev) => {
              const lastMessage = prev[prev.length - 1];
              const nextMessage: ChatMessage = {
                id: `${Date.now()}-assistant-resume`,
                role: 'assistant',
                content: payload.delta,
                timestamp: new Date(),
                toolCalls: payload.toolResults.length > 0
                  ? toToolCalls(payload.toolResults.filter(isRecord))
                  : undefined,
              };

              if (lastMessage?.role === 'assistant' && lastMessage.content === '等待权限确认后继续执行。') {
                return [
                  ...prev.slice(0, -1),
                  {
                    ...lastMessage,
                    content: payload.delta,
                    timestamp: new Date(),
                    toolCalls: nextMessage.toolCalls,
                  },
                ];
              }

              return [...prev, nextMessage];
            });
          }

          if (payload.done) {
            setIsLoading(false);
            pendingAssistantIdRef.current = null;
            void loadSessionMessages(payload.sessionId);
            void loadSessions();
          }
        });
      } catch {
        unlisten = null;
      }
    };

    void subscribe();
    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, [loadSessionMessages, loadSessions]);

  useEffect(() => {
    let unlisten: (() => void) | null = null;

    const subscribe = async () => {
      try {
        unlisten = await listenPermissionRequest((payload) => {
          setPermRequest({
            id: payload.id,
            toolName: payload.toolName,
            args: isRecord(payload.args) ? payload.args : {},
            riskLevel: payload.riskLevel,
            description: payload.description,
          });
        });
      } catch {
        unlisten = null;
      }
    };

    void subscribe();
    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, []);

  const handleCreateSession = useCallback(async () => {
    try {
      const agents = await agentList();
      const defaultAgent = (agents ?? [])[0];
      if (!defaultAgent) {
        return;
      }

      const created = await sessionCreate(defaultAgent.id);
      activeSessionRef.current = created.id;
      setActiveSession(created.id);
      await loadSessions();
      await loadSessionMessages(created.id);
    } catch {
      setSessions([]);
      setActiveSession('');
      setMessages([]);
    }
  }, [loadSessionMessages, loadSessions]);

  const handleSend = async () => {
    const text = input.trim();
    if (!text || isLoading) {
      return;
    }

    let sessionId = activeSessionRef.current;
    if (!sessionId) {
      try {
        const agents = await agentList();
        const defaultAgent = (agents ?? [])[0];
        if (!defaultAgent) {
          return;
        }

        const created = await sessionCreate(defaultAgent.id);
        sessionId = created.id;
        activeSessionRef.current = created.id;
        setActiveSession(created.id);
        await loadSessions();
      } catch {
        setSessions([]);
        return;
      }
    }

    setIsLoading(true);

    if (editingMessageId) {
      try {
        await sessionRewriteMessage(sessionId, editingMessageId, text);
        setInput('');
        await chatRetry(sessionId, editingMessageId);
      } catch {
        setIsLoading(false);
      }
      setEditingMessageId(null);
      return;
    }

    const assistantId = `${Date.now()}-assistant`;
    pendingAssistantIdRef.current = assistantId;

    const userMsg: ChatMessage = {
      id: Date.now().toString(),
      role: 'user',
      content: text,
      timestamp: new Date(),
    };

    setMessages((prev) => [
      ...prev,
      userMsg,
      {
        id: assistantId,
        role: 'assistant',
        content: '',
        timestamp: new Date(),
      },
    ]);
    setInput('');

    try {
      await chatSend(sessionId, text);
    } catch {
      setIsLoading(false);
      pendingAssistantIdRef.current = null;
      setMessages((prev) => prev.map((item) => {
        if (item.id !== assistantId) {
          return item;
        }
        return {
          ...item,
          content: '消息发送失败。',
        };
      }));
    }
  };

  const handleCopy = useCallback(async (content: string) => {
    try {
      await navigator.clipboard.writeText(content);
    } catch {
    }
  }, []);

  const handleRetry = useCallback(async (messageId: string) => {
    if (!activeSessionRef.current || isLoading) {
      return;
    }
    setIsLoading(true);
    pendingAssistantIdRef.current = null;
    try {
      await chatRetry(activeSessionRef.current, messageId);
    } catch {
      setIsLoading(false);
      void loadSessionMessages(activeSessionRef.current);
    }
  }, [isLoading, loadSessionMessages]);

  const handleEdit = useCallback((messageId: string, content: string) => {
    setEditingMessageId(messageId);
    setInput(content);
  }, []);

  const handlePermissionApprove = useCallback((id: string) => {
    void (async () => {
      setIsLoading(true);
      try {
        await permissionRespond(id, true);
      } catch {
        setIsLoading(false);
      }
      setPermRequest(null);
    })();
  }, []);

  const handlePermissionDeny = useCallback((id: string) => {
    void (async () => {
      setIsLoading(true);
      try {
        await permissionRespond(id, false);
      } catch {
        setIsLoading(false);
      }
      setPermRequest(null);
    })();
  }, []);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      void handleSend();
    }
  };

  return (
    <div className="chat-page">
      <div className="chat-main">
        <div className="chat-messages">
          {messages.map((msg) => (
            <div key={msg.id} className={`chat-msg chat-msg-${msg.role} animate-in`}>
              <div className="chat-msg-avatar">
                {msg.role === 'user' ? <User size={18} /> : <Bot size={18} />}
              </div>
              <div className="chat-msg-body">
                <div className="chat-msg-header">
                  <span className="chat-msg-name">
                    {msg.role === 'user' ? '你' : 'CodeForge Agent'}
                  </span>
                  <span className="chat-msg-time">
                    {msg.timestamp.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' })}
                  </span>
                </div>
                <div className="chat-msg-content">
                  {msg.content.split('\n').map((line, i) => {
                    if (line.startsWith('```')) {
                      return <pre key={i}><code>{line.replace(/```\w*/, '')}</code></pre>;
                    }
                    return <p key={i} dangerouslySetInnerHTML={{
                      __html: line
                        .replace(/\*\*(.*?)\*\*/g, '<strong>$1</strong>')
                        .replace(/`(.*?)`/g, '<code>$1</code>')
                        .replace(/- /g, '• ')
                    }} />;
                  })}
                </div>
                {msg.toolCalls && msg.toolCalls.length > 0 && (
                  <div className="chat-tools">
                    <div className="chat-tools-header">
                      <Wrench size={14} />
                      <span>工具调用</span>
                    </div>
                    {msg.toolCalls.map((tc, i) => (
                      <div key={i} className="chat-tool-item">
                        <span className="chat-tool-name">{tc.name}</span>
                        <span className="chat-tool-args">{tc.args}</span>
                        {tc.result && <span className="chat-tool-result">→ {tc.result}</span>}
                      </div>
                    ))}
                  </div>
                )}
                <div className="chat-msg-actions">
                  <button type="button" className="btn btn-ghost btn-sm" onClick={() => void handleCopy(msg.content)}><Copy size={14} /></button>
                  <button type="button" className="btn btn-ghost btn-sm" onClick={() => void handleRetry(msg.id)}><RotateCcw size={14} /></button>
                  {msg.role === 'user' && <button type="button" className="btn btn-ghost btn-sm" onClick={() => handleEdit(msg.id, msg.content)}><Pencil size={14} /></button>}
                </div>
              </div>
            </div>
          ))}
          {isLoading && (
            <div className="chat-msg chat-msg-assistant animate-in">
              <div className="chat-msg-avatar"><Bot size={18} /></div>
              <div className="chat-msg-body">
                <div className="chat-typing">
                  <Loader2 size={16} className="chat-typing-spinner" />
                  <span>Agent 正在思考...</span>
                </div>
              </div>
            </div>
          )}
          <div ref={messagesEndRef} />
        </div>

        <div className="chat-input-area">
          <div className="chat-input-wrapper">
            <button className="btn btn-ghost btn-icon"><Paperclip size={18} /></button>
            <textarea
              className="chat-input"
              placeholder={editingMessageId ? '编辑消息后按发送重新生成回复...' : '输入消息，或粘贴代码进行审查...'}
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={handleKeyDown}
              rows={1}
            />
            <button className="chat-send-btn" onClick={() => void handleSend()} disabled={!input.trim() || isLoading}>
              <Send size={18} />
            </button>
          </div>
          <div className="chat-input-hint">
            <Sparkles size={12} />
            <span>支持 Markdown · Shift+Enter 换行 · 工具调用自动执行</span>
            <button className="btn btn-ghost btn-sm" style={{ marginLeft: 'auto' }} onClick={() => setShowPanel(!showPanel)}>
              {showPanel ? <PanelRightClose size={14} /> : <PanelRightOpen size={14} />}
            </button>
          </div>
        </div>
      </div>

      {showPanel && (
        <div className="chat-sidebar">
          <div className="chat-sidebar-section">
            <div className="chat-sidebar-header">
              <Layers size={16} />
              <span>上下文窗口</span>
            </div>
            <div className="context-meter">
              <div className="context-meter-bar">
                <div className="context-meter-fill" style={{ width: `${Math.max(0, Math.min(100, contextPercent))}%` }} />
              </div>
              <div className="context-meter-info">
                <span>{(contextUsed / 1000).toFixed(1)}K / {(contextMax / 1000).toFixed(0)}K tokens</span>
                <span>{contextPercent}%</span>
              </div>
            </div>
            <div className="context-details">
              <div><Gauge size={12} /> 压缩: 自动</div>
              <div><Bot size={12} /> Model: {model || '未设置'}</div>
            </div>
          </div>

          <div className="chat-sidebar-section">
            <div className="chat-sidebar-header">
              <Settings2 size={16} />
              <span>模型配置</span>
            </div>
            <div className="chat-config">
              <label>Provider</label>
              <select value={provider} onChange={e => setProvider(e.target.value)}>
                {providerOptions.length === 0 && <option value="">未配置 Provider</option>}
                {providerOptions.map((item) => (
                  <option key={item.id} value={item.id}>{item.name}</option>
                ))}
              </select>
            </div>
            <div className="chat-config">
              <label>Model</label>
              <select value={model} onChange={e => setModel(e.target.value)}>
                {modelOptions.length === 0 && <option value="">未配置模型</option>}
                {modelOptions.map((item) => (
                  <option key={item} value={item}>{item}</option>
                ))}
              </select>
            </div>
            <div className="chat-config">
              <label>Embedding Provider</label>
              <select value={embeddingProvider} onChange={e => applyEmbeddingProvider(e.target.value, providerOptions)}>
                <option value={EMBEDDING_ENV_PROVIDER_ID}>环境配置</option>
                <option value="">未绑定 Provider</option>
                {providerOptions.map((item) => (
                  <option key={item.id} value={item.id}>{item.name}</option>
                ))}
              </select>
            </div>
            <div className="chat-config">
              <label>Embedding Endpoint</label>
              <input
                type="text"
                value={embeddingEndpoint}
                onChange={e => setEmbeddingEndpoint(e.target.value)}
                style={{ width: '100%', padding: '0.4rem', background: 'var(--bg-input)', border: '1px solid var(--border-primary)', borderRadius: 4, color: 'var(--text-primary)' }}
              />
            </div>
            <div className="chat-config">
              <label>Embedding Model</label>
              <select value={embeddingModel} onChange={e => setEmbeddingModel(e.target.value)}>
                {embeddingModelOptions.length === 0 && <option value="">未配置模型</option>}
                {embeddingModelOptions.map((item) => (
                  <option key={item} value={item}>{item}</option>
                ))}
              </select>
            </div>
            <div className="chat-config">
              <label>Embedding Key</label>
              <input
                type="password"
                value={embeddingApiKey}
                onChange={e => setEmbeddingApiKey(e.target.value)}
                style={{ width: '100%', padding: '0.4rem', background: 'var(--bg-input)', border: '1px solid var(--border-primary)', borderRadius: 4, color: 'var(--text-primary)' }}
              />
            </div>
            <div className="chat-config">
              <div className="chat-config-row">
                <label><Thermometer size={12} /> Temperature</label>
                <span>{temperature.toFixed(2)}</span>
              </div>
              <input type="range" min="0" max="2" step="0.05" value={temperature} onChange={e => setTemperature(parseFloat(e.target.value))} />
            </div>
            <div className="chat-config">
              <div className="chat-config-row">
                <label>Top P</label>
                <span>{topP.toFixed(2)}</span>
              </div>
              <input type="range" min="0" max="1" step="0.05" value={topP} onChange={e => setTopP(parseFloat(e.target.value))} />
            </div>
            <div className="chat-config">
              <div className="chat-config-row">
                <label>Max Tokens</label>
                <input type="number" value={maxTokens} onChange={e => setMaxTokens(parseInt(e.target.value, 10) || 0)} style={{ width: 80, textAlign: 'right', padding: '2px 6px', fontSize: 12, background: 'var(--bg-input)', border: '1px solid var(--border-primary)', borderRadius: 4, color: 'var(--text-primary)' }} />
              </div>
            </div>
            <div className="chat-config">
              <div className="chat-config-row">
                <label>Streaming</label>
                <button className={`toggle-btn ${streaming ? 'on' : ''}`} onClick={() => setStreaming(!streaming)}>
                  <span className="toggle-knob" />
                </button>
              </div>
            </div>
          </div>

          <div className="chat-sidebar-section">
            <div className="chat-sidebar-header">
              <MessageSquare size={16} />
              <span>会话历史</span>
              <button className="btn btn-ghost btn-icon btn-sm" style={{ marginLeft: 'auto' }} onClick={() => void handleCreateSession()}>
                <Plus size={14} />
              </button>
            </div>
            <div className="session-list">
              {sessions.map(s => (
                <div key={s.id} className={`session-item ${activeSession === s.id ? 'active' : ''}`} onClick={() => setActiveSession(s.id)}>
                  <div className="session-title">{s.title}</div>
                  <div className="session-meta">{s.messageCount} 条消息 · {(s.tokenUsed / 1000).toFixed(1)}K tokens</div>
                </div>
              ))}
            </div>
          </div>
        </div>
      )}

      <PermissionDialog
        request={permRequest}
        onApprove={handlePermissionApprove}
        onDeny={handlePermissionDeny}
      />
    </div>
  );
}
