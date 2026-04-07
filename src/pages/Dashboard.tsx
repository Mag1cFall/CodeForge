import { useState, useEffect, useCallback } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  MessageSquare, Bot, Wrench, Zap, BookOpen, Activity,
  ArrowUpRight, Clock, TrendingUp, Flame
} from 'lucide-react';
import { useAppPreferences } from '../lib/app-preferences';
import { agentList, toolList, toolUsageCounts, skillList, knowledgeRepos, logList, TraceLog } from '../lib/backend';
import './Dashboard.css';

interface StatItem {
  label: string;
  value: string;
  icon: typeof Bot;
  color: 'blue' | 'purple' | 'green' | 'orange';
  trend: string;
  path: string;
}

interface ActivityItem {
  agent: string;
  action: string;
  time: string;
  status: 'success' | 'pending' | 'error';
}

const defaultStats: StatItem[] = [
  { label: '活跃 Agent', value: '0', icon: Bot, color: 'blue', trend: '+0', path: '/agents' },
  { label: '工具调用', value: '0', icon: Wrench, color: 'purple', trend: '+0', path: '/logs' },
  { label: '技能数', value: '0', icon: Zap, color: 'green', trend: '+0', path: '/skills' },
  { label: '知识库文档', value: '0', icon: BookOpen, color: 'orange', trend: '+0', path: '/knowledge' },
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

const isRecord = (value: unknown): value is Record<string, unknown> => {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
};

const readString = (value: Record<string, unknown>, key: string): string => {
  const target = value[key];
  return typeof target === 'string' ? target : '';
};

const toRelativeTime = (timeText: string): string => {
  const date = new Date(timeText);
  if (Number.isNaN(date.getTime())) {
    return '刚刚';
  }

  const diffMs = Date.now() - date.getTime();
  const diffMinutes = Math.max(0, Math.floor(diffMs / 60000));
  if (diffMinutes < 1) {
    return '刚刚';
  }
  if (diffMinutes < 60) {
    return `${diffMinutes} 分钟前`;
  }

  const diffHours = Math.floor(diffMinutes / 60);
  if (diffHours < 24) {
    return `${diffHours} 小时前`;
  }

  const diffDays = Math.floor(diffHours / 24);
  return `${diffDays} 天前`;
};

const toActivity = (log: TraceLog): ActivityItem => {
  const payload = isRecord(log.payload) ? log.payload : {};
  const action = readString(payload, 'action') || readString(payload, 'name') || readString(payload, 'content') || log.kind;
  const agent = readString(payload, 'agent') || log.kind;
  const statusValue = readString(payload, 'status').toLowerCase();
  const status: ActivityItem['status'] =
    statusValue === 'error' || log.kind.includes('error')
      ? 'error'
      : statusValue === 'pending'
        ? 'pending'
        : 'success';

  return {
    agent,
    action,
    time: toRelativeTime(log.createdAt),
    status,
  };
};

export default function Dashboard() {
  const navigate = useNavigate();
  const { t } = useAppPreferences();
  const [stats, setStats] = useState<StatItem[]>(defaultStats);
  const [recentActivities, setRecentActivities] = useState<ActivityItem[]>([]);

  const loadDashboard = useCallback(async () => {
    const [agents, tools, counts, skills, repos, logs] = await Promise.all([
      agentList().catch(() => []),
      toolList().catch(() => []),
      toolUsageCounts().catch(() => []),
      skillList().catch(() => []),
      knowledgeRepos().catch(() => []),
      logList(12).catch(() => []),
    ]);

    const activeAgents = agents.filter((item) => item.status === 'running').length;
    const toolCalls = counts.reduce((sum, item) => sum + item.calls, 0);
    const skillCount = skills.length;
    const knowledgeChunks = repos.reduce((sum, item) => sum + item.chunkCount, 0);

    setStats([
      { label: '活跃 Agent', value: activeAgents.toString(), icon: Bot, color: 'blue', trend: '+0', path: '/agents' },
      { label: '工具调用', value: toolCalls.toLocaleString('zh-CN'), icon: Wrench, color: 'purple', trend: `+${tools.length}`, path: '/logs' },
      { label: '技能数', value: skillCount.toString(), icon: Zap, color: 'green', trend: '+0', path: '/skills' },
      { label: '知识库文档', value: knowledgeChunks.toLocaleString('zh-CN'), icon: BookOpen, color: 'orange', trend: '+0', path: '/knowledge' },
    ]);

    setRecentActivities(logs.slice(0, 5).map(toActivity));
  }, []);

  useEffect(() => {
    void loadDashboard();
  }, [loadDashboard]);

  return (
    <div className="dashboard animate-in">
      <div className="page-header">
        <div className="dashboard-header-row">
          <div>
            <h1>
              <Flame size={28} style={{ verticalAlign: 'middle', marginRight: 10 }} />
              {t('route.dashboard')}
            </h1>
            <p>{t('page.dashboard.desc')}</p>
          </div>
        </div>
      </div>

        <div className="stats-grid">
          {stats.map((stat) => (
          <div
            key={stat.label}
            className={`stat-card card card-glow stat-${stat.color}`}
            style={{ cursor: 'pointer' }}
            role="button"
            tabIndex={0}
            onClick={() => navigate(stat.path)}
            onKeyDown={(event) => {
              if (event.key === 'Enter' || event.key === ' ') {
                event.preventDefault();
                navigate(stat.path);
              }
            }}
          >
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
                key={`${a.agent}-${a.time}-${i}`}
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
