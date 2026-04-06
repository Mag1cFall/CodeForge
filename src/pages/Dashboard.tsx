import { useNavigate } from 'react-router-dom';
import {
  MessageSquare, Bot, Wrench, Zap, BookOpen, Activity,
  ArrowUpRight, Clock, TrendingUp, Flame
} from 'lucide-react';
import './Dashboard.css';

const stats = [
  { label: '活跃 Agent', value: '5', icon: Bot, color: 'blue', trend: '+2' },
  { label: '工具调用', value: '1,247', icon: Wrench, color: 'purple', trend: '+89' },
  { label: '技能数', value: '12', icon: Zap, color: 'green', trend: '+3' },
  { label: '知识库文档', value: '356', icon: BookOpen, color: 'orange', trend: '+24' },
];

const recentActivities = [
  { agent: 'Reviewer', action: '审查了 src/main.rs', time: '2 分钟前', status: 'success' },
  { agent: 'Refactorer', action: '重构建议: extract_function()', time: '5 分钟前', status: 'success' },
  { agent: 'Researcher', action: '搜索最佳实践: Error Handling', time: '8 分钟前', status: 'pending' },
  { agent: 'Orchestrator', action: '编排代码审查任务', time: '12 分钟前', status: 'success' },
  { agent: 'Executor', action: '运行 cargo test', time: '15 分钟前', status: 'error' },
];

const quickActions = [
  { label: '开始代码审查', icon: MessageSquare, gradient: 'var(--gradient-primary)', path: '/review' },
  { label: '添加 MCP 服务', icon: Activity, gradient: 'var(--gradient-secondary)', path: '/mcp' },
  { label: '配置新模型', icon: TrendingUp, gradient: 'var(--gradient-success)', path: '/providers' },
];

const archRoutes: Record<string, string> = {
  Agent: '/agents',
  MCP: '/mcp',
  Skill: '/skills',
  Tool: '/tools',
  RAG: '/knowledge',
  Harness: '/review',
};

export default function Dashboard() {
  const navigate = useNavigate();
  return (
    <div className="dashboard animate-in">
      <div className="page-header">
        <div className="dashboard-header-row">
          <div>
            <h1>
              <Flame size={28} style={{ verticalAlign: 'middle', marginRight: 10 }} />
              欢迎使用 CodeForge
            </h1>
            <p>基于多Agent协作的代码智能分析与最佳实践挖掘平台</p>
          </div>
        </div>
      </div>

      <div className="stats-grid">
        {stats.map((stat) => (
          <div key={stat.label} className={`stat-card card card-glow stat-${stat.color}`}>
            <div className="stat-card-header">
              <div className={`stat-icon stat-icon-${stat.color}`}>
                <stat.icon size={20} />
              </div>
              <div className="stat-trend">
                <ArrowUpRight size={14} />
                {stat.trend}
              </div>
            </div>
            <div className="stat-value">{stat.value}</div>
            <div className="stat-label">{stat.label}</div>
          </div>
        ))}
      </div>

      <div className="dashboard-grid">
        <div className="card dashboard-activity">
          <h3>
            <Clock size={18} />
            最近活动
          </h3>
          <div className="activity-list">
            {recentActivities.map((a, i) => (
              <div 
                key={i} 
                className="activity-item" 
                style={{ animationDelay: `${i * 0.05}s`, cursor: 'pointer' }}
                onClick={() => navigate('/logs')}
              >
                <div className={`activity-dot activity-dot-${a.status}`} />
                <div className="activity-info">
                  <span className="activity-agent">{a.agent}</span>
                  <span className="activity-action">{a.action}</span>
                </div>
                <span className="activity-time">{a.time}</span>
              </div>
            ))}
          </div>
        </div>

        <div className="dashboard-quick">
          <h3>快速操作</h3>
          <div className="quick-actions">
            {quickActions.map((action, i) => (
              <button key={i} className="quick-action-btn" onClick={() => navigate(action.path)}>
                <div className="quick-action-icon" style={{ background: action.gradient }}>
                  <action.icon size={20} color="white" />
                </div>
                <span>{action.label}</span>
                <ArrowUpRight size={14} className="quick-action-arrow" />
              </button>
            ))}
          </div>

          <div className="card architecture-card">
            <h4>系统架构</h4>
            <div className="arch-modules">
              {['Agent', 'MCP', 'Skill', 'Tool', 'RAG', 'Harness'].map((m) => (
                <div key={m} className="arch-module" style={{ cursor: 'pointer' }} onClick={() => navigate(archRoutes[m] || '/')}>{m}</div>
              ))}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
