import { useState, useEffect, useCallback } from 'react';
import { Bot, Plus, Play, Square, Settings2, Cpu, Trash2, Save, X, Lock } from 'lucide-react';
import { useAppPreferences } from '../lib/app-preferences';
import {
  agentCreate, agentList, agentUpdate, agentDelete,
  agentStart, agentStop, AgentRecord, AgentConfigInput,
  providerList, ProviderSummary,
} from '../lib/backend';
import './PageCommon.css';

export default function Agents() {
  const { t } = useAppPreferences();
  const [agents, setAgents] = useState<AgentRecord[]>([]);
  const [providers, setProviders] = useState<ProviderSummary[]>([]);
  const [showForm, setShowForm] = useState(false);
  const [editId, setEditId] = useState<string | null>(null);
  const [editIsSystem, setEditIsSystem] = useState(false);
  const [form, setForm] = useState({ name: '', instructions: '', model: '', tools: '' });

  const loadAgents = useCallback(async () => {
    try {
      const data = await agentList();
      setAgents(data ?? []);
    } catch {
      setAgents([]);
    }
  }, []);

  const loadProviders = useCallback(async () => {
    try {
      const data = await providerList();
      setProviders(data ?? []);
    } catch {
      setProviders([]);
    }
  }, []);

  useEffect(() => {
    void loadAgents();
    void loadProviders();
  }, [loadAgents, loadProviders]);

  const allModels = Array.from(new Set(providers.flatMap(p => [p.model, ...p.models]).filter(Boolean)));

  const toggleStatus = async (id: string, currentStatus: string) => {
    try {
      if (currentStatus === 'running') {
        await agentStop(id);
      } else {
        await agentStart(id);
      }
      await loadAgents();
    } catch { /* best effort */ }
  };

  const handleDelete = async (agent: AgentRecord) => {
    if (agent.isSystem) return;
    try {
      await agentDelete(agent.id);
      await loadAgents();
    } catch { /* best effort */ }
  };

  const handleSave = () => {
    if (!form.name.trim() || !form.model.trim()) return;

    const config: AgentConfigInput = {
      name: form.name.trim(),
      instructions: form.instructions.trim() || null,
      model: form.model.trim(),
      tools: form.tools.split(',').map(t => t.trim()).filter(Boolean),
    };

    void (async () => {
      try {
        if (editId) {
          await agentUpdate(editId, config);
        } else {
          await agentCreate(config);
        }
        await loadAgents();
      } catch { /* best effort */ }
      setShowForm(false);
      setEditId(null);
      setEditIsSystem(false);
    })();
  };

  const openForm = (agent?: AgentRecord) => {
    if (agent) {
      setEditId(agent.id);
      setEditIsSystem(agent.isSystem);
      setForm({
        name: agent.name,
        instructions: agent.instructions || '',
        model: agent.model,
        tools: agent.tools.join(', '),
      });
    } else {
      setEditId(null);
      setEditIsSystem(false);
      const defaultModel = providers.find(p => p.isDefault)?.model || allModels[0] || '';
      setForm({ name: '', instructions: '', model: defaultModel, tools: '' });
    }
    setShowForm(true);
  };

  const cancelForm = () => {
    setShowForm(false);
    setEditId(null);
    setEditIsSystem(false);
  };

  return (
    <div className="animate-in">
      <div className="page-header">
        <h1><Bot size={28} style={{ verticalAlign: 'middle', marginRight: 8 }} /> {t('route.agents')}</h1>
        <p>{t('page.agents.desc')}</p>
      </div>

      <div className="page-toolbar">
        <button type="button" className="btn btn-primary" onClick={() => { if (showForm) cancelForm(); else openForm(); }}>
          {showForm ? <><X size={16} /> 取消</> : <><Plus size={16} /> 创建 Agent</>}
        </button>
      </div>

      {showForm && (
        <div className="card" style={{ marginBottom: 20 }}>
          <h4 style={{ margin: '0 0 16px', fontSize: 15, fontWeight: 600 }}>
            {editId ? (editIsSystem ? '编辑系统 Agent' : '编辑 Agent') : '创建新 Agent'}
            {editIsSystem && <Lock size={14} style={{ marginLeft: 6, opacity: 0.5 }} />}
          </h4>
          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 16 }}>
            <div>
              <label htmlFor="agent-name">名称</label>
              <input
                id="agent-name"
                value={form.name}
                onChange={e => setForm({...form, name: e.target.value})}
                disabled={editIsSystem}
                style={{ width: '100%' }}
              />
              {editIsSystem && <span style={{ fontSize: 11, color: 'var(--text-tertiary)' }}>系统 Agent 名称不可修改</span>}
            </div>
            <div>
              <label htmlFor="agent-model">模型</label>
              {allModels.length > 0 ? (
                <select
                  id="agent-model"
                  value={form.model}
                  onChange={e => setForm({...form, model: e.target.value})}
                  style={{ width: '100%', padding: '0.5rem', background: 'var(--bg-card)', border: '1px solid var(--border-color)', color: 'var(--text-primary)', borderRadius: '4px' }}
                >
                  {allModels.map(m => <option key={m} value={m}>{m}</option>)}
                  {form.model && !allModels.includes(form.model) && (
                    <option value={form.model}>{form.model} (自定义)</option>
                  )}
                </select>
              ) : (
                <input
                  id="agent-model"
                  value={form.model}
                  onChange={e => setForm({...form, model: e.target.value})}
                  placeholder="输入模型名称"
                  style={{ width: '100%' }}
                />
              )}
            </div>
            <div style={{ gridColumn: 'span 2' }}>
              <label htmlFor="agent-instructions">系统指令</label>
              <textarea
                id="agent-instructions"
                value={form.instructions}
                onChange={e => setForm({...form, instructions: e.target.value})}
                placeholder="定义此 Agent 的行为和能力范围..."
                style={{ width: '100%', minHeight: '80px', padding: '0.5rem', background: 'var(--bg-card)', border: '1px solid var(--border-color)', color: 'var(--text-primary)', borderRadius: '4px', resize: 'vertical' }}
              />
            </div>
            <div style={{ gridColumn: 'span 2' }}>
              <label htmlFor="agent-tools">启用工具 (用逗号分隔)</label>
              <input
                id="agent-tools"
                value={form.tools}
                onChange={e => setForm({...form, tools: e.target.value})}
                placeholder="read_file, search_code, find_code_smells"
                style={{ width: '100%' }}
              />
            </div>
          </div>
          <div style={{ display: 'flex', gap: 10, marginTop: 16 }}>
            <button type="button" className="btn btn-primary" onClick={handleSave}>
              <Save size={14} /> {editId ? '保存修改' : '创建'}
            </button>
            <button type="button" className="btn btn-secondary" onClick={cancelForm}>取消</button>
          </div>
        </div>
      )}

      <div className="card-grid">
        {agents.map((agent) => (
          <div key={agent.id} className="card card-glow agent-card">
            <div className="agent-card-header">
              <div className="agent-avatar"><Bot size={22} /></div>
              <div>
                <h4 style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                  {agent.name}
                  {agent.isSystem && (
                    <span className="badge badge-blue" style={{ fontSize: 10, padding: '1px 6px' }}>系统</span>
                  )}
                </h4>
                <span className="text-secondary" style={{ fontSize: 12 }}>{agent.instructions || '系统默认'}</span>
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
              {!agent.isSystem && (
                <button type="button" className="btn btn-sm btn-ghost" style={{ color: 'var(--accent-red)' }} onClick={() => handleDelete(agent)}><Trash2 size={14} /></button>
              )}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
