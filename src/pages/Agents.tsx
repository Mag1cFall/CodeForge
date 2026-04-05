import { Bot, Plus, Play, Square, Settings2, Cpu } from 'lucide-react';
import './PageCommon.css';

const agents = [
  { name: 'Orchestrator', role: '编排者', status: 'running', model: 'claude-opus-4-6', tasks: 12 },
  { name: 'Reviewer', role: '审查者', status: 'running', model: 'claude-sonnet-4-6', tasks: 89 },
  { name: 'Refactorer', role: '重构者', status: 'idle', model: 'gpt-4o', tasks: 34 },
  { name: 'Researcher', role: '研究者', status: 'running', model: 'gemini-2.5-pro', tasks: 56 },
  { name: 'Executor', role: '执行者', status: 'stopped', model: 'deepseek-v3', tasks: 23 },
];

export default function Agents() {
  return (
    <div className="animate-in">
      <div className="page-header">
        <h1>🤖 Agent 管理</h1>
        <p>配置和管理 AI Agent 角色、指令和工具权限</p>
      </div>

      <div className="page-toolbar">
        <button className="btn btn-primary"><Plus size={16} /> 创建 Agent</button>
      </div>

      <div className="card-grid">
        {agents.map((agent) => (
          <div key={agent.name} className="card card-glow agent-card">
            <div className="agent-card-header">
              <div className="agent-avatar"><Bot size={22} /></div>
              <div>
                <h4>{agent.name}</h4>
                <span className="text-secondary">{agent.role}</span>
              </div>
              <span className={`badge badge-${agent.status === 'running' ? 'green' : agent.status === 'idle' ? 'orange' : 'red'}`}>
                {agent.status === 'running' ? '运行中' : agent.status === 'idle' ? '空闲' : '已停止'}
              </span>
            </div>
            <div className="agent-card-meta">
              <div><Cpu size={14} /> {agent.model}</div>
              <div>已完成 {agent.tasks} 个任务</div>
            </div>
            <div className="agent-card-actions">
              {agent.status !== 'running' ? (
                <button className="btn btn-sm btn-secondary"><Play size={14} /> 启动</button>
              ) : (
                <button className="btn btn-sm btn-secondary"><Square size={14} /> 停止</button>
              )}
              <button className="btn btn-sm btn-ghost"><Settings2 size={14} /></button>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
