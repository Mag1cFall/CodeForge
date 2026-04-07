import { useState, useRef, useEffect, useCallback } from 'react';
import { useSearchParams } from 'react-router-dom';
import {
  Send, Bot, User, Loader2, Paperclip, Sparkles, RotateCcw, Copy, Wrench, Pencil,
  PanelRightOpen, PanelRightClose, Plus, MessageSquare, Gauge, Layers,
  Settings2, Thermometer, Trash2
} from 'lucide-react';
import PermissionDialog, { PermissionRequest } from '../components/PermissionDialog';
import ConfirmDialog from '../components/ConfirmDialog';
import {
  agentList,
  embeddingConfigGet,
  chatSend,
  chatRetry,
  listenChatChunk,
  listenChatProgress,
  listenChatToolResult,
  listenPermissionRequest,
  permissionPending,
  permissionRespond,
  providerList,
  ProviderSummary,
  sessionRewriteMessage,
  sessionCreate,
  sessionDelete,
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
  isToolEvent?: boolean;
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

interface DeleteConfirmState {
  id: string;
  name: string;
}

interface InlineEditingState {
  id: string;
  draft: string;
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
    const args = item.args;
    const result = item.result;
    return {
      name: readString(item, 'name') || 'tool',
      args: typeof args === 'string' ? args : JSON.stringify(args ?? {}, null, 2),
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

const isVisibleMessage = (message: SessionMessage): boolean => {
  return message.role === 'user' || message.role === 'assistant';
};

const toChatMessage = (message: SessionMessage): ChatMessage => {
  const role: ChatMessage['role'] = message.role === 'user' ? 'user' : 'assistant';
  const toolCalls = toToolCalls(message.toolCalls.filter(isRecord));

  return {
    id: message.id,
    role,
    content: message.content,
    timestamp: parseTimestamp(message.createdAt),
    isToolEvent: message.role === 'assistant' && message.toolCalls.length > 0 && message.content.startsWith('Tool result:'),
    toolCalls: toolCalls.length > 0 ? toolCalls : undefined,
  };
};

export default function Chat() {
  const [searchParams, setSearchParams] = useSearchParams();
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [sessions, setSessions] = useState<Session[]>([]);
  const [providerOptions, setProviderOptions] = useState<ProviderSummary[]>([]);
  const [input, setInput] = useState('');
  const [inlineEditing, setInlineEditing] = useState<InlineEditingState | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [showPanel, setShowPanel] = useState(true);
  const [activeSession, setActiveSession] = useState('');
  const [permRequest, setPermRequest] = useState<PermissionRequest | null>(null);
  const [permissionBusyId, setPermissionBusyId] = useState<string | null>(null);
  const [progressMessage, setProgressMessage] = useState('');
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
  const [deleteConfirm, setDeleteConfirm] = useState<DeleteConfirmState | null>(null);
  const [sidebarWidth, setSidebarWidth] = useState(300);
  const [isSidebarResizing, setIsSidebarResizing] = useState(false);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);
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
    setInlineEditing(null);
    setProgressMessage('');
  }, [activeSession]);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);

  const autoResizeInput = useCallback(() => {
    const target = inputRef.current;
    if (!target) {
      return;
    }
    target.style.height = '38px';
    const next = Math.min(Math.max(target.scrollHeight, 38), 260);
    target.style.height = `${next}px`;
  }, []);

  useEffect(() => {
    autoResizeInput();
  }, [input, autoResizeInput]);

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
      if (!permissionBusyId) {
        setPermRequest(null);
      }
      setProgressMessage('');
      return;
    }

    try {
      const [data, pending] = await Promise.all([
        sessionMessages(sessionId),
        permissionPending(sessionId),
      ]);
      setMessages((data ?? []).filter(isVisibleMessage).map(toChatMessage));
      if (!permissionBusyId && sessionId === activeSessionRef.current) {
        if (pending) {
          setPermRequest({
            id: pending.id,
            toolName: pending.toolName,
            args: isRecord(pending.args) ? pending.args : {},
            riskLevel: pending.riskLevel,
            description: pending.description,
          });
          setProgressMessage(`等待权限确认：${pending.toolName}`);
        } else {
          setPermRequest(null);
          setProgressMessage('');
        }
      }
    } catch {
      setMessages([]);
      if (!permissionBusyId && sessionId === activeSessionRef.current) {
        setPermRequest(null);
      }
      setProgressMessage('');
    }
  }, [permissionBusyId]);

  const loadSessions = useCallback(async () => {
    try {
      const records = await sessionList();
      const next = await Promise.all(
        (records ?? []).map(async (record) => {
          let messageCount = 0;
          try {
            const messages = await sessionMessages(record.id);
            messageCount = (messages ?? []).filter(isVisibleMessage).length;
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

  const resetProviderState = useCallback(() => {
    setProviderOptions([]);
    setProvider('');
    setEmbeddingProvider('');
    setEmbeddingEndpoint('');
    setEmbeddingModel('');
    setEmbeddingApiKey('');
  }, []);

  const loadProviders = useCallback(async () => {
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
      resetProviderState();
    }
  }, [loadEmbeddingConfig, resetProviderState]);

  useEffect(() => {
    void loadProviders();
    void loadSessions();
  }, [loadProviders, loadSessions]);

  useEffect(() => {
    if (searchParams.get('autofix') !== '1') return;
    const stored = sessionStorage.getItem('codeforge_autofix_prompt');
    if (!stored) return;
    sessionStorage.removeItem('codeforge_autofix_prompt');
    setSearchParams({}, { replace: true });
    setInput(stored);
    const timer = setTimeout(() => {
      const btn = document.querySelector('.chat-send-btn') as HTMLButtonElement | null;
      btn?.click();
    }, 600);
    return () => clearTimeout(timer);
  }, [searchParams, setSearchParams]);

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
      if (!permissionBusyId) {
        setPermRequest(null);
      }
      return;
    }
    void loadSessionMessages(activeSession);
  }, [activeSession, loadSessionMessages, permissionBusyId]);

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
              const resumeId = `resume-${payload.sessionId}`;
              const nextToolCalls = payload.toolResults.length > 0
                ? toToolCalls(payload.toolResults.filter(isRecord))
                : undefined;
              const nextMessage: ChatMessage = {
                id: resumeId,
                role: 'assistant',
                content: payload.delta,
                timestamp: new Date(),
                toolCalls: nextToolCalls,
              };

              if (lastMessage?.id === resumeId) {
                return [
                  ...prev.slice(0, -1),
                  {
                    ...lastMessage,
                    content: `${lastMessage.content}${payload.delta}`,
                    timestamp: new Date(),
                    toolCalls: payload.done ? nextToolCalls : lastMessage.toolCalls,
                  },
                ];
              }

              if (lastMessage?.role === 'assistant' && lastMessage.content.startsWith('等待权限确认')) {
                return [
                  ...prev.slice(0, -1),
                  {
                    ...lastMessage,
                    content: payload.delta,
                    timestamp: new Date(),
                    toolCalls: nextToolCalls,
                  },
                ];
              }

              return [...prev, nextMessage];
            });
          }

          if (payload.done) {
            setIsLoading(false);
            setProgressMessage('');
            const removedId = pendingAssistantIdRef.current;
            pendingAssistantIdRef.current = null;
            if (removedId) {
              setMessages((prev) => prev.filter((item) => item.id !== removedId));
            }
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
        unlisten = await listenChatProgress((payload) => {
          if (payload.sessionId !== activeSessionRef.current) {
            return;
          }
          setProgressMessage(payload.message || '');
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

  useEffect(() => {
    let unlisten: (() => void) | null = null;

    const subscribe = async () => {
      try {
        unlisten = await listenChatToolResult((payload) => {
          if (payload.sessionId !== activeSessionRef.current) {
            return;
          }
          if (!isRecord(payload.tool)) {
            return;
          }

          const toolCalls = toToolCalls([payload.tool]);
          if (toolCalls.length === 0) {
            return;
          }

          setMessages((prev) => [
            ...prev,
            {
              id: `tool-live-${readString(payload.tool, 'id') || Date.now().toString()}-${Math.random().toString(36).slice(2, 7)}`,
              role: 'assistant',
              content: 'Tool result:',
              isToolEvent: true,
              timestamp: new Date(),
              toolCalls,
            },
          ]);
          setProgressMessage(`已执行工具：${toolCalls[0].name}`);
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
          setProgressMessage(`等待权限确认：${payload.toolName}`);
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

  const handleRequestDeleteSession = useCallback((sessionId: string) => {
    if (!sessionId || isLoading) {
      return;
    }
    const target = sessions.find((item) => item.id === sessionId);
    setDeleteConfirm({
      id: sessionId,
      name: target?.title || '当前会话',
    });
  }, [isLoading, sessions]);

  const handleCloseDeleteConfirm = useCallback(() => {
    setDeleteConfirm(null);
  }, []);

  const handleConfirmDelete = useCallback(async () => {
    if (!deleteConfirm || isLoading) {
      return;
    }

    const action = deleteConfirm;
    setDeleteConfirm(null);

    try {
      await sessionDelete(action.id);
      if (activeSessionRef.current === action.id) {
        activeSessionRef.current = '';
        setActiveSession('');
        setMessages([]);
        setInlineEditing(null);
        pendingAssistantIdRef.current = null;
      }
      await loadSessions();
    } catch {
    }
  }, [deleteConfirm, isLoading, loadSessions]);

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
    setProgressMessage('模型正在思考...');

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
      await chatSend(sessionId, text, {
        providerId: provider || undefined,
        model: model || undefined,
        temperature: temperature,
        topP: topP,
        maxTokens: maxTokens,
        stream: streaming,
      });
    } catch (error) {
      setIsLoading(false);
      setProgressMessage('');
      pendingAssistantIdRef.current = null;
      const errorText = error instanceof Error ? error.message : String(error);
      setMessages((prev) => prev.map((item) => {
        if (item.id !== assistantId) {
          return item;
        }
        return {
          ...item,
          content: `消息发送失败：${errorText}`,
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

    const retryAssistantId = `retry-${Date.now()}`;
    setIsLoading(true);
    setProgressMessage('模型正在思考...');
    pendingAssistantIdRef.current = retryAssistantId;
    setMessages((prev) => [
      ...prev,
      {
        id: retryAssistantId,
        role: 'assistant',
        content: '',
        timestamp: new Date(),
      },
    ]);

    try {
      await chatRetry(activeSessionRef.current, messageId);
    } catch {
      setIsLoading(false);
      setProgressMessage('');
      pendingAssistantIdRef.current = null;
      setMessages((prev) => prev.filter((item) => item.id !== retryAssistantId));
      void loadSessionMessages(activeSessionRef.current);
    }
  }, [isLoading, loadSessionMessages]);

  const handleEdit = useCallback((messageId: string, content: string) => {
    if (isLoading) {
      return;
    }
    setInlineEditing({
      id: messageId,
      draft: content,
    });
  }, [isLoading]);

  const handleCancelInlineEdit = useCallback(() => {
    if (isLoading) {
      return;
    }
    setInlineEditing(null);
  }, [isLoading]);

  const handleSubmitInlineEdit = useCallback(async () => {
    if (!inlineEditing || !activeSessionRef.current || isLoading) {
      return;
    }

    const draft = inlineEditing.draft.trim();
    if (!draft) {
      return;
    }

    const sessionId = activeSessionRef.current;
    const messageId = inlineEditing.id;
    const retryAssistantId = `edit-retry-${Date.now()}`;

    setIsLoading(true);
    setProgressMessage('模型正在思考...');
    pendingAssistantIdRef.current = retryAssistantId;
    setInlineEditing(null);
    setMessages((prev) => {
      const rewritten = prev.map((item) => {
        if (item.id === messageId) {
          return {
            ...item,
            content: draft,
            timestamp: new Date(),
          };
        }
        return item;
      });
      return [
        ...rewritten,
        {
          id: retryAssistantId,
          role: 'assistant',
          content: '',
          timestamp: new Date(),
        },
      ];
    });

    try {
      await sessionRewriteMessage(sessionId, messageId, draft);
      await chatRetry(sessionId, messageId);
    } catch {
      setIsLoading(false);
      setProgressMessage('');
      pendingAssistantIdRef.current = null;
      setMessages((prev) => prev.filter((item) => item.id !== retryAssistantId));
      void loadSessionMessages(sessionId);
    }
  }, [inlineEditing, isLoading, loadSessionMessages]);

  const createPermissionPendingMessage = useCallback(() => {
    const messageId = `permission-${Date.now()}`;
    pendingAssistantIdRef.current = messageId;
    setMessages((prev) => [
      ...prev,
      {
        id: messageId,
        role: 'assistant',
        content: '',
        timestamp: new Date(),
      },
    ]);
    return messageId;
  }, []);

  const handlePermissionApprove = useCallback((id: string) => {
    if (permissionBusyId === id) {
      return;
    }
    void (async () => {
      const pendingId = createPermissionPendingMessage();
      setPermissionBusyId(id);
      setIsLoading(true);
      setProgressMessage('已确认权限，正在继续执行...');
      setPermRequest((current) => (current?.id === id ? null : current));
      try {
        await permissionRespond(id, true);
      } catch (error) {
        setIsLoading(false);
        pendingAssistantIdRef.current = null;
        setProgressMessage('权限处理失败。');
        setMessages((prev) => {
          const removedPending = prev.filter((item) => item.id !== pendingId);
          return [
            ...removedPending,
            {
              id: `permission-error-${Date.now()}`,
              role: 'assistant',
              content: `权限处理失败：${error instanceof Error ? error.message : String(error)}`,
              timestamp: new Date(),
            },
          ];
        });
      } finally {
        setPermissionBusyId(null);
      }
    })();
  }, [createPermissionPendingMessage, permissionBusyId]);

  const handlePermissionDeny = useCallback((id: string) => {
    if (permissionBusyId === id) {
      return;
    }
    void (async () => {
      const pendingId = createPermissionPendingMessage();
      setPermissionBusyId(id);
      setIsLoading(true);
      setProgressMessage('已拒绝权限，正在继续执行...');
      setPermRequest((current) => (current?.id === id ? null : current));
      try {
        await permissionRespond(id, false);
      } catch (error) {
        setIsLoading(false);
        pendingAssistantIdRef.current = null;
        setProgressMessage('权限处理失败。');
        setMessages((prev) => {
          const removedPending = prev.filter((item) => item.id !== pendingId);
          return [
            ...removedPending,
            {
              id: `permission-error-${Date.now()}`,
              role: 'assistant',
              content: `权限处理失败：${error instanceof Error ? error.message : String(error)}`,
              timestamp: new Date(),
            },
          ];
        });
      } finally {
        setPermissionBusyId(null);
      }
    })();
  }, [createPermissionPendingMessage, permissionBusyId]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      void handleSend();
    }
  };

  const handleSidebarResizeStart = useCallback((event: React.MouseEvent<HTMLButtonElement>) => {
    event.preventDefault();
    const startX = event.clientX;
    const startWidth = sidebarWidth;

    const onMouseMove = (moveEvent: MouseEvent) => {
      const delta = startX - moveEvent.clientX;
      const nextWidth = Math.min(Math.max(startWidth + delta, 240), 520);
      setSidebarWidth(nextWidth);
    };

    const onMouseUp = () => {
      setIsSidebarResizing(false);
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
      document.removeEventListener('mousemove', onMouseMove);
      document.removeEventListener('mouseup', onMouseUp);
    };

    setIsSidebarResizing(true);
    document.body.style.cursor = 'col-resize';
    document.body.style.userSelect = 'none';
    document.addEventListener('mousemove', onMouseMove);
    document.addEventListener('mouseup', onMouseUp);
  }, [sidebarWidth]);

  const confirmTitle = '删除会话';
  const confirmDescription = deleteConfirm
    ? `确认删除“${deleteConfirm.name}”？此操作不可恢复。`
    : '';
  const confirmActionLabel = '删除会话';

  return (
    <div className="chat-page">
      <div className="chat-main">
        <div className="chat-messages">
          {messages.map((msg) => (
            <div key={msg.id} className={`chat-msg chat-msg-${msg.role} ${msg.isToolEvent ? 'chat-msg-tool-event' : ''} animate-in`}>
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
                  {msg.isToolEvent ? (
                    <p className="chat-tool-event-title">工具执行完成</p>
                  ) : msg.role === 'user' && inlineEditing?.id === msg.id ? (
                    <div className="chat-inline-edit">
                      <textarea
                        className="chat-inline-edit-input"
                        value={inlineEditing.draft}
                        onChange={(event) => {
                          const draft = event.target.value;
                          setInlineEditing((current) => {
                            if (!current || current.id !== msg.id) {
                              return current;
                            }
                            return {
                              ...current,
                              draft,
                            };
                          });
                        }}
                        rows={4}
                      />
                      <div className="chat-inline-edit-actions">
                        <button type="button" className="btn btn-ghost btn-sm" onClick={handleCancelInlineEdit} disabled={isLoading}>取消</button>
                        <button type="button" className="btn btn-primary btn-sm" onClick={() => void handleSubmitInlineEdit()} disabled={isLoading || !inlineEditing.draft.trim()}>保存并重试</button>
                      </div>
                    </div>
                  ) : msg.content.trim().length === 0 && msg.role === 'assistant' && isLoading && !(msg.toolCalls && msg.toolCalls.length > 0) ? (
                    <div className="chat-typing">
                      <Loader2 size={16} className="chat-typing-spinner" />
                      <span>{progressMessage || 'Agent 正在思考...'}</span>
                    </div>
                  ) : (
                    msg.content.split('\n').map((line, i) => {
                      if (line.startsWith('```')) {
                        return <pre key={i}><code>{line.replace(/```\w*/, '')}</code></pre>;
                      }
                      return <p key={i} dangerouslySetInnerHTML={{
                        __html: line
                          .replace(/\*\*(.*?)\*\*/g, '<strong>$1</strong>')
                          .replace(/`(.*?)`/g, '<code>$1</code>')
                          .replace(/- /g, '• ')
                      }} />;
                    })
                  )}
                </div>
                {msg.toolCalls && msg.toolCalls.length > 0 && (
                  <div className="chat-tools">
                    <div className="chat-tools-header">
                      <Wrench size={14} />
                      <span>工具调用</span>
                    </div>
                    {msg.toolCalls.map((tc) => (
                      <details key={`${msg.id}-${tc.name}-${tc.args.slice(0, 40)}`} className="chat-tool-item" open={msg.isToolEvent}>
                        <summary className="chat-tool-summary">
                          <span className="chat-tool-name">{tc.name}</span>
                          <span className="chat-tool-meta">查看参数与输出</span>
                        </summary>
                        <div className="chat-tool-block">
                          <div className="chat-tool-block-title">参数</div>
                          <pre className="chat-tool-code">{tc.args}</pre>
                        </div>
                        {tc.result && (
                          <div className="chat-tool-block">
                            <div className="chat-tool-block-title">输出</div>
                            <pre className="chat-tool-code">{tc.result}</pre>
                          </div>
                        )}
                      </details>
                    ))}
                  </div>
                )}
                {msg.role === 'assistant' && isLoading && msg.id === messages[messages.length - 1]?.id && (msg.content.trim().length > 0 || (msg.toolCalls && msg.toolCalls.length > 0)) && (
                  <div className="chat-typing" style={{ marginTop: '12px' }}>
                    <Loader2 size={14} className="chat-typing-spinner" />
                    <span>{progressMessage || 'Agent 正在思考...'}</span>
                  </div>
                )}
                <div className="chat-msg-actions">
                  <button type="button" className="btn btn-ghost btn-sm" onClick={() => void handleCopy(msg.content)}><Copy size={14} /></button>
                  <button type="button" className="btn btn-ghost btn-sm" onClick={() => void handleRetry(msg.id)}><RotateCcw size={14} /></button>
                  {msg.role === 'user' && inlineEditing?.id !== msg.id && <button type="button" className="btn btn-ghost btn-sm" onClick={() => handleEdit(msg.id, msg.content)}><Pencil size={14} /></button>}
                </div>
              </div>
            </div>
          ))}
          <div ref={messagesEndRef} />
        </div>

        <div className="chat-input-area">
          <div className="chat-input-wrapper">
            <button type="button" className="btn btn-ghost btn-icon"><Paperclip size={18} /></button>
            <textarea
              ref={inputRef}
              className="chat-input"
              placeholder="输入消息，或粘贴代码进行审查..."
              value={input}
              onChange={(e) => {
                setInput(e.target.value);
                requestAnimationFrame(autoResizeInput);
              }}
              onKeyDown={handleKeyDown}
              rows={2}
            />
            <button type="button" className="chat-send-btn" onClick={() => void handleSend()} disabled={!input.trim() || isLoading}>
              <Send size={18} />
            </button>
          </div>
          <div className="chat-input-hint">
            <Sparkles size={12} />
            <span>支持 Markdown · Shift+Enter 换行 · 工具调用自动执行</span>
            <button type="button" className="btn btn-ghost btn-sm" style={{ marginLeft: 'auto' }} onClick={() => setShowPanel(!showPanel)}>
              {showPanel ? <PanelRightClose size={14} /> : <PanelRightOpen size={14} />}
            </button>
          </div>
        </div>
      </div>

      {showPanel && (
        <>
          <button
            type="button"
            className={`chat-sidebar-resizer ${isSidebarResizing ? 'active' : ''}`}
            onMouseDown={handleSidebarResizeStart}
            title="拖拽调整侧边栏宽度"
            aria-label="拖拽调整侧边栏宽度"
          />
          <div className="chat-sidebar" style={{ width: sidebarWidth }}>
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
                <button type="button" className={`toggle-btn ${streaming ? 'on' : ''}`} onClick={() => setStreaming(!streaming)}>
                  <span className="toggle-knob" />
                </button>
              </div>
            </div>
          </div>

          <div className="chat-sidebar-section">
            <div className="chat-sidebar-header">
              <MessageSquare size={16} />
              <span>会话历史</span>
              <button type="button" className="btn btn-ghost btn-icon btn-sm" style={{ marginLeft: 'auto' }} onClick={() => void handleCreateSession()}>
                <Plus size={14} />
              </button>
            </div>
            <div className="session-list">
              {sessions.map(s => (
                <div
                  key={s.id}
                  className={`session-item ${activeSession === s.id ? 'active' : ''}`}
                  role="button"
                  tabIndex={0}
                  onClick={() => setActiveSession(s.id)}
                  onKeyDown={(event) => {
                    if (event.key === 'Enter' || event.key === ' ') {
                      event.preventDefault();
                      setActiveSession(s.id);
                    }
                  }}
                >
                  <div className="session-head">
                    <div className="session-title">{s.title}</div>
                    <button
                      type="button"
                      className="session-delete-btn"
                      onClick={(event) => {
                        event.stopPropagation();
                        handleRequestDeleteSession(s.id);
                      }}
                      title="删除会话"
                      aria-label="删除会话"
                    >
                      <Trash2 size={12} />
                    </button>
                  </div>
                  <div className="session-meta">{s.messageCount} 条消息 · {(s.tokenUsed / 1000).toFixed(1)}K tokens</div>
                </div>
              ))}
            </div>
          </div>
          </div>
        </>
      )}

      <PermissionDialog
        request={permRequest}
        processing={permissionBusyId !== null}
        onApprove={handlePermissionApprove}
        onDeny={handlePermissionDeny}
      />
      <ConfirmDialog
        open={Boolean(deleteConfirm)}
        title={confirmTitle}
        description={confirmDescription}
        confirmLabel={confirmActionLabel}
        cancelLabel="取消"
        danger
        onConfirm={() => void handleConfirmDelete()}
        onCancel={handleCloseDeleteConfirm}
      />
    </div>
  );
}
