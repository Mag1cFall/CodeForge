import { useState, useEffect, useCallback } from 'react';
import { Plug, Plus, CheckCircle2, XCircle, RefreshCw, ExternalLink } from 'lucide-react';
import { useAppPreferences } from '../lib/app-preferences';
import { mcpServerList, mcpServerAdd, mcpServerTools, McpServerRecord } from '../lib/backend';
import './PageCommon.css';

interface ServerView extends McpServerRecord {
  status: 'connected' | 'disconnected';
  tools: number;
  resources: number;
}

export default function MCP() {
  const { t } = useAppPreferences();
  const [serverList, setServerList] = useState<ServerView[]>([]);
  const [showForm, setShowForm] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [form, setForm] = useState({ name: '', transport: 'stdio', cmdOrUrl: '', autoDiscover: true });

  const loadServers = useCallback(async () => {
    try {
      const servers = await mcpServerList();
      const next = await Promise.all(
        (servers ?? []).map(async (server) => {
          let toolCount = 0;
          const status: ServerView['status'] = server.enabled ? 'connected' : 'disconnected';
          try {
            const tools = await mcpServerTools(server.id);
            toolCount = tools.length;
          } catch {
            toolCount = 0;
          }

          return {
            ...server,
            status,
            tools: toolCount,
            resources: 0,
          };
        })
      );
      setServerList(next);
    } catch {
      setServerList([]);
    }
  }, []);

  useEffect(() => {
    void loadServers();
  }, [loadServers]);

  const handleRefresh = async () => {
    setRefreshing(true);
    try {
      await loadServers();
    } finally {
      setRefreshing(false);
    }
  };

  const handleCreate = async () => {
    if (form.name && form.cmdOrUrl) {
      try {
        await mcpServerAdd({
          name: form.name,
          transport: form.transport,
          command: form.transport === 'stdio' ? form.cmdOrUrl : null,
          url: form.transport === 'sse' ? form.cmdOrUrl : null,
          args: [],
          env: {},
          headers: {},
          enabled: true,
        });
        setShowForm(false);
        setForm({ name: '', transport: 'stdio', cmdOrUrl: '', autoDiscover: true });
        await loadServers();
      } catch {
        setServerList([]);
      }
    }
  };

  return (
    <div className="animate-in">
      <div className="page-header">
        <h1><Plug size={28} style={{ verticalAlign: 'middle', marginRight: 8 }} /> {t('route.mcp')}</h1>
        <p>{t('page.mcp.desc')}</p>
      </div>
      <div className="page-toolbar">
        <button className="btn btn-primary" onClick={() => setShowForm(!showForm)}>
          <Plus size={16} /> {showForm ? '取消' : '添加 MCP Server'}
        </button>
        <button className="btn btn-secondary" onClick={handleRefresh}>
          <RefreshCw size={16} className={refreshing ? 'spin' : ''} style={refreshing ? { animation: 'spin 1s linear infinite' } : {}} /> 
          {refreshing ? '刷新中...' : '刷新连接'}
        </button>
      </div>

      {showForm && (
        <div className="card" style={{ marginBottom: 20 }}>
          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 16 }}>
            <div>
              <label>名称</label>
              <input value={form.name} onChange={e => setForm({...form, name: e.target.value})} style={{ width: '100%' }} />
            </div>
            <div>
              <label>传输方式</label>
              <select value={form.transport} onChange={e => setForm({...form, transport: e.target.value})} style={{ width: '100%', padding: '0.5rem', background: 'var(--bg-card)', border: '1px solid var(--border-color)', color: 'var(--text-primary)', borderRadius: '4px' }}>
                <option value="stdio">stdio</option>
                <option value="sse">SSE</option>
              </select>
            </div>
            <div style={{ gridColumn: 'span 2' }}>
              <label>{form.transport === 'stdio' ? '命令 (含路径和参数)' : 'URL'}</label>
              <input value={form.cmdOrUrl} onChange={e => setForm({...form, cmdOrUrl: e.target.value})} placeholder={form.transport === 'stdio' ? '例如: node /path/to/mcp/index.js' : 'http://localhost:8080/mcp'} style={{ width: '100%' }} />
            </div>
          </div>
          <div style={{ display: 'flex', gap: 10, marginTop: 16, alignItems: 'center' }}>
            <label style={{ display: 'flex', alignItems: 'center', gap: 6, margin: 0, cursor: 'pointer' }}>
              <input type="checkbox" checked={form.autoDiscover} onChange={e => setForm({...form, autoDiscover: e.target.checked})} />
              自动发现工具和资源
            </label>
            <div style={{ flex: 1 }} />
            <button className="btn btn-primary" onClick={handleCreate}>添加服务</button>
          </div>
        </div>
      )}

      <div className="table-card card">
        <table className="data-table">
          <thead>
            <tr>
              <th>名称</th>
              <th>传输方式</th>
              <th>状态</th>
              <th>工具数</th>
              <th>资源数</th>
              <th>操作</th>
            </tr>
          </thead>
          <tbody>
            {serverList.map((s) => (
              <tr key={s.id}>
                <td style={{ fontFamily: 'var(--font-mono)', fontWeight: 600 }}>{s.name}</td>
                <td><span className="badge badge-purple">{s.transport}</span></td>
                <td>
                  {s.status === 'connected' ? (
                    <span className="badge badge-green"><CheckCircle2 size={12} /> 已连接</span>
                  ) : (
                    <span className="badge badge-red"><XCircle size={12} /> 断开</span>
                  )}
                </td>
                <td>{s.tools}</td>
                <td>{s.resources}</td>
                <td>
                  <button className="btn btn-sm btn-ghost" onClick={() => alert(`Details for ${s.name}`)}><ExternalLink size={14} /></button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
