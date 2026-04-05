import { useState, useRef, useEffect } from 'react';
import { Send, Bot, User, Loader2, Paperclip, Sparkles, RotateCcw, Copy, Wrench } from 'lucide-react';
import './Chat.css';

interface ChatMessage {
  id: string;
  role: 'user' | 'assistant' | 'tool';
  content: string;
  timestamp: Date;
  toolCalls?: { name: string; args: string; result?: string }[];
  thinking?: boolean;
}

const initialMessages: ChatMessage[] = [
  {
    id: '1',
    role: 'assistant',
    content: '你好！我是 **CodeForge Agent**，你的 AI 代码助手。\n\n我可以帮你：\n- 🔍 **审查代码** — 发现潜在问题和改进空间\n- 🛠️ **重构建议** — 提供最佳实践推荐\n- 📚 **知识检索** — 基于 RAG 语义搜索代码库\n- ⚡ **执行任务** — 通过工具调用完成复杂操作\n\n请输入你的需求，或者直接粘贴代码片段让我审查。',
    timestamp: new Date(),
  }
];

export default function Chat() {
  const [messages, setMessages] = useState<ChatMessage[]>(initialMessages);
  const [input, setInput] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);

  const handleSend = async () => {
    if (!input.trim() || isLoading) return;
    const userMsg: ChatMessage = {
      id: Date.now().toString(),
      role: 'user',
      content: input,
      timestamp: new Date(),
    };
    setMessages(prev => [...prev, userMsg]);
    setInput('');
    setIsLoading(true);

    setTimeout(() => {
      const toolMsg: ChatMessage = {
        id: (Date.now() + 1).toString(),
        role: 'assistant',
        content: '正在分析你的代码...\n\n```rust\n// 检测到以下问题:\n// 1. unwrap() 未处理错误 → 建议使用 ? 运算符\n// 2. 函数复杂度过高 (CC=12) → 建议拆分\n// 3. 变量命名不规范: x, y → 建议语义化命名\n```\n\n**建议修改**:\n- 使用 `Result<T, E>` 替代 `unwrap()`\n- 将 `process_data()` 拆分为 `validate_input()` + `transform_data()` + `output_result()`\n- 重命名变量以增加可读性',
        timestamp: new Date(),
        toolCalls: [
          { name: 'analyze_ast', args: '{"file": "src/main.rs"}', result: 'CC=12, lines=156' },
          { name: 'find_code_smells', args: '{"pattern": "unwrap()"}', result: '3 occurrences found' },
        ],
      };
      setMessages(prev => [...prev, toolMsg]);
      setIsLoading(false);
    }, 1500);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  return (
    <div className="chat-page">
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
                <button className="btn btn-ghost btn-sm" title="复制">
                  <Copy size={14} />
                </button>
                <button className="btn btn-ghost btn-sm" title="重新生成">
                  <RotateCcw size={14} />
                </button>
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
          <button className="btn btn-ghost btn-icon" title="上传文件">
            <Paperclip size={18} />
          </button>
          <textarea
            ref={inputRef}
            className="chat-input"
            placeholder="输入消息，或粘贴代码进行审查..."
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={handleKeyDown}
            rows={1}
          />
          <button
            className="chat-send-btn"
            onClick={handleSend}
            disabled={!input.trim() || isLoading}
          >
            <Send size={18} />
          </button>
        </div>
        <div className="chat-input-hint">
          <Sparkles size={12} />
          <span>支持 Markdown · Shift+Enter 换行 · 工具调用自动执行</span>
        </div>
      </div>
    </div>
  );
}
