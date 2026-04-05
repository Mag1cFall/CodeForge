import { Plus, CheckCircle2, Key, Globe } from 'lucide-react';
import './PageCommon.css';

const providers = [
  { name: 'OpenAI', endpoint: 'https://api.openai.com/v1', models: ['gpt-5.4', 'gpt-5.4-mini'], status: 'active', keySet: true },
  { name: 'Anthropic', endpoint: 'https://api.anthropic.com/v1', models: ['claude-opus-4-6', 'claude-sonnet-4-6'], status: 'active', keySet: true },
  { name: 'DeepSeek', endpoint: 'https://api.deepseek.com/v1', models: ['deepseek-v3.2', 'deepseek-coder-v3'], status: 'active', keySet: true },
  { name: 'Ollama', endpoint: 'http://localhost:11434', models: ['qwen-3:72b', 'llama-4'], status: 'inactive', keySet: false },
];

export default function Providers() {
  return (
    <div className="animate-in">
      <div className="page-header">
        <h1>🏗️ 模型配置</h1>
        <p>配置 LLM Provider 连接，支持 OpenAI / Anthropic / DeepSeek / Ollama 等</p>
      </div>

      <div className="page-toolbar">
        <button className="btn btn-primary"><Plus size={16} /> 添加 Provider</button>
      </div>

      <div className="card-grid">
        {providers.map((p) => (
          <div key={p.name} className="card card-glow">
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 16 }}>
              <h4 style={{ fontSize: 16, fontWeight: 700 }}>{p.name}</h4>
              {p.status === 'active'
                ? <span className="badge badge-green"><CheckCircle2 size={12} /> 可用</span>
                : <span className="badge badge-red">未激活</span>
              }
            </div>
            <div style={{ fontSize: 12, color: 'var(--text-tertiary)', fontFamily: 'var(--font-mono)', marginBottom: 12 }}>
              <Globe size={12} style={{ verticalAlign: 'middle', marginRight: 4 }} />
              {p.endpoint}
            </div>
            <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap', marginBottom: 12 }}>
              {p.models.map((m) => (
                <span key={m} className="badge badge-purple">{m}</span>
              ))}
            </div>
            <div style={{ fontSize: 12, color: p.keySet ? 'var(--accent-green-light)' : 'var(--text-tertiary)' }}>
              <Key size={12} style={{ verticalAlign: 'middle', marginRight: 4 }} />
              {p.keySet ? 'API Key 已配置' : '未设置 API Key'}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
