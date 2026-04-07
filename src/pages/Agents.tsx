import { useState, useEffect, useCallback } from 'react';
import { Bot, Plus, Play, Square, Settings2, Cpu } from 'lucide-react';
import { useAppPreferences } from '../lib/app-preferences';
import { agentCreate, agentList, agentStart, agentStop, AgentRecord } from '../lib/backend';
import './PageCommon.css';

export default function Agents() {
  const { t } = useAppPreferences();
  const [agents, setAgents] = useState<AgentRecord[]>([]);
  const [showForm, setShowForm] = useState(false);
  const [editId, setEditId] = useState<string | null>(null);
  const [form, setForm] = useState({ name: '', instructions: '', model: 'claude-sonnet-4-5', tools: ['fs', 'shell'] });
  
  const loadAgents = useCallback(async () => {
    try {
      const data = await agentList();
      setAgents(data ?? []);
    } catch {
      setAgents([]);
    }
  }, []);

  useEffect(() => {
    void loadAgents();
  }, [loadAgents]);

  const toggleStatus = async (id: string, currentStatus: string) => {
    try {
      if (currentStatus === 'running') {
        await agentStop(id);
      } else {
        await agentStart(id);
      }
      await loadAgents();
    } catch {
      setAgents([]);
    }
  };

  const handleSave = () => {
    if (editId) {
      setShowForm(false);
      setEditId(null);
      return;
    }

    if (!form.name.trim() || !form.model.trim()) {
      return;
    }

    void (async () => {
      try {
        await agentCreate({
          name: form.name.trim(),
          instructions: form.instructions.trim() || null,
          model: form.model.trim(),
          tools: form.tools,
        });
        await loadAgents();
      } catch {
        setAgents([]);
      }
      setShowForm(false);
      setEditId(null);
    })();
  };

  const openForm = (agent?: AgentRecord) => {
    if (agent) {
      setEditId(agent.id);
      setForm({ name: agent.name, instructions: agent.instructions || '', model: agent.model, tools: agent.tools });
    } else {
      setEditId(null);
      setForm({ name: '', instructions: '', model: 'claude-sonnet-4-5', tools: ['fs', 'shell'] });
    }
    setShowForm(true);
  };

  return (
    <div className="animate-in">
      <div className="page-header">
        <h1><Bot size={28} style={{ verticalAlign: 'middle', marginRight: 8 }} /> {t('route.agents')}</h1>
        <p>{t('page.agents.desc')}</p>
      </div>

      <div className="page-toolbar">
        <button type="button" className="btn btn-primary" onClick={() => setShowForm(!showForm)}>
          <Plus size={16} /> {showForm ? '取消' : '创建 Agent'}
        </button>
      </div>

      {showForm && (
        <div className="card" style={{ marginBottom: 20 }}>
          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 16 }}>
            <div>
              <label htmlFor="agent-name">名称</label>
              <input id="agent-name" value={form.name} onChange={e => setForm({...form, name: e.target.value})} style={{ width: '100%' }} />
            </div>
            <div>
              <label htmlFor="agent-model">模型</label>
              <select id="agent-model" value={form.model} onChange={e => setForm({...form, model: e.target.value})} style={{ width: '100%', padding: '0.5rem', background: 'var(--bg-card)', border: '1px solid var(--border-color)', color: 'var(--text-primary)', borderRadius: '4px' }}>
                <option value="claude-sonnet-4-5">Claude Sonnet 4.5</option>
                <option value="gpt-5.4">GPT-5.4</option>
                <option value="gemini-3.1-pro">Gemini 3.1 Pro</option>
              </select>
            </div>
            <div style={{ gridColumn: 'span 2' }}>
              <label htmlFor="agent-instructions">系统指令</label>
              <textarea id="agent-instructions" value={form.instructions} onChange={e => setForm({...form, instructions: e.target.value})} style={{ width: '100%', minHeight: '80px', padding: '0.5rem', background: 'var(--bg-card)', border: '1px solid var(--border-color)', color: 'var(--text-primary)', borderRadius: '4px', resize: 'vertical' }} />
            </div>
            <div style={{ gridColumn: 'span 2' }}>
              <label htmlFor="agent-tools">启用工具 (用逗号分隔)</label>
              <input id="agent-tools" value={form.tools.join(',')} onChange={e => setForm({...form, tools: e.target.value.split(',').map(t => t.trim()).filter(Boolean)})} style={{ width: '100%' }} />
            </div>
          </div>
          <div style={{ display: 'flex', gap: 10, marginTop: 16 }}>
            <button type="button" className="btn btn-primary" onClick={handleSave}>保存当前 Agent</button>
          </div>
        </div>
      )}

      <div className="card-grid">
        {agents.map((agent) => (
          <div key={agent.id} className="card card-glow agent-card">
            <div className="agent-card-header">
              <div className="agent-avatar"><Bot size={22} /></div>
              <div>
                <h4>{agent.name}</h4>
                <span className="text-secondary">{agent.instructions || '系统默认'}</span>
              </div>
              <span className={`badge badge-${agent.status === 'running' ? 'green' : agent.status === 'idle' ? 'orange' : 'red'}`}>
                {agent.status === 'running' ? '运行中' : agent.status === 'idle' ? '空闲' : '已停止'}
              </span>
            </div>
            <div className="agent-card-meta">
              <div><Cpu size={14} /> {agent.model}</div>
              <div>已使用 {agent.tools.length} 个工具</div>
            </div>
            <div className="agent-card-actions">
              {agent.status !== 'running' ? (
                <button type="button" className="btn btn-sm btn-secondary" onClick={() => toggleStatus(agent.id, agent.status)}><Play size={14} /> 启动</button>
              ) : (
                <button type="button" className="btn btn-sm btn-secondary" onClick={() => toggleStatus(agent.id, agent.status)}><Square size={14} /> 停止</button>
              )}
              <button type="button" className="btn btn-sm btn-ghost" onClick={() => openForm(agent)}><Settings2 size={14} /></button>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
