import { NavLink } from 'react-router-dom';
import {
  LayoutDashboard, MessageSquare, SearchCode, Bot,
  Wrench, Plug, Zap, BookOpen, Server, ScrollText,
  Settings, PanelLeftClose, PanelLeft, Flame
} from 'lucide-react';
import '../styles/sidebar.css';

const navSections = [
  {
    label: '核心',
    items: [
      { path: '/', icon: LayoutDashboard, label: '仪表盘' },
      { path: '/chat', icon: MessageSquare, label: '对话' },
      { path: '/review', icon: SearchCode, label: '代码审查' },
    ]
  },
  {
    label: 'Agent 框架',
    items: [
      { path: '/agents', icon: Bot, label: 'Agent 管理' },
      { path: '/tools', icon: Wrench, label: '工具注册' },
      { path: '/mcp', icon: Plug, label: 'MCP 服务' },
      { path: '/skills', icon: Zap, label: '技能市场' },
    ]
  },
  {
    label: '数据',
    items: [
      { path: '/knowledge', icon: BookOpen, label: '知识库' },
      { path: '/providers', icon: Server, label: '模型配置' },
      { path: '/logs', icon: ScrollText, label: '执行日志' },
    ]
  },
  {
    label: '系统',
    items: [
      { path: '/settings', icon: Settings, label: '设置' },
    ]
  }
];

interface SidebarProps {
  collapsed: boolean;
  onToggle: () => void;
}

export default function Sidebar({ collapsed, onToggle }: SidebarProps) {
  return (
    <aside className={`sidebar ${collapsed ? 'collapsed' : ''}`}>
      <div className="sidebar-header">
        <div className="sidebar-logo">
          <Flame size={18} />
        </div>
        <div className="sidebar-brand">
          <h2>CodeForge</h2>
          <span>炼码 · AI Code Agent</span>
        </div>
      </div>

      <nav className="sidebar-nav">
        {navSections.map((section) => (
          <div key={section.label}>
            <div className="nav-section-label">{section.label}</div>
            {section.items.map((item) => (
              <NavLink
                key={item.path}
                to={item.path}
                className={({ isActive }) =>
                  `nav-item ${isActive ? 'active' : ''}`
                }
                end={item.path === '/'}
              >
                <item.icon size={18} />
                <span className="nav-item-label">{item.label}</span>
              </NavLink>
            ))}
          </div>
        ))}
      </nav>

      <div className="sidebar-footer">
        <button type="button" className="sidebar-toggle" onClick={onToggle}>
          {collapsed ? <PanelLeft size={18} /> : <PanelLeftClose size={18} />}
          {!collapsed && <span>收起侧栏</span>}
        </button>
      </div>
    </aside>
  );
}
