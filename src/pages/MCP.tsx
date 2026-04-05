import { Plug, Plus, CheckCircle2, XCircle, RefreshCw, ExternalLink } from 'lucide-react';
import './PageCommon.css';

const servers = [
  { name: 'eslint-mcp', transport: 'stdio', status: 'connected', tools: 3, resources: 1 },
  { name: 'github-mcp', transport: 'sse', status: 'connected', tools: 8, resources: 5 },
  { name: 'postgres-mcp', transport: 'stdio', status: 'disconnected', tools: 6, resources: 2 },
];

export default function MCP() {
  return (
    <div className="animate-in">
      <div className="page-header">
        <h1>🔌 MCP 服务</h1>
        <p>管理 Model Context Protocol 服务器，扩展 Agent 能力边界</p>
      </div>
      <div className="page-toolbar">
        <button className="btn btn-primary"><Plus size={16} /> 添加 MCP Server</button>
        <button className="btn btn-secondary"><RefreshCw size={16} /> 刷新连接</button>
      </div>

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
            {servers.map((s) => (
              <tr key={s.name}>
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
                  <button className="btn btn-sm btn-ghost"><ExternalLink size={14} /></button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
