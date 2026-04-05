import { useLocation } from 'react-router-dom';
import { Bell, Search } from 'lucide-react';
import '../styles/layout.css';

const routeNames: Record<string, string> = {
  '/': '仪表盘',
  '/chat': '对话',
  '/review': '代码审查',
  '/agents': 'Agent 管理',
  '/tools': '工具注册',
  '/mcp': 'MCP 服务',
  '/skills': '技能市场',
  '/knowledge': '知识库',
  '/providers': '模型配置',
  '/logs': '执行日志',
  '/settings': '设置',
};

export default function TopBar() {
  const location = useLocation();
  const currentName = routeNames[location.pathname] || '未知页面';

  return (
    <header className="topbar">
      <div className="topbar-left">
        <div className="topbar-breadcrumb">
          <span>CodeForge</span>
          <span>/</span>
          <span className="current">{currentName}</span>
        </div>
      </div>
      <div className="topbar-right">
        <button className="btn btn-ghost btn-icon" title="搜索">
          <Search size={18} />
        </button>
        <button className="btn btn-ghost btn-icon" title="通知">
          <Bell size={18} />
        </button>
        <div className="topbar-status">
          <div className="topbar-status-dot" />
          <span>系统就绪</span>
        </div>
      </div>
    </header>
  );
}
