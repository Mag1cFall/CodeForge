import { useState, useEffect, useRef } from 'react';
import { useLocation } from 'react-router-dom';
import { Bell, Search, Sun, Moon } from 'lucide-react';
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
  
  const [isDark, setIsDark] = useState(true);
  const [isSearchOpen, setIsSearchOpen] = useState(false);
  const [isBellOpen, setIsBellOpen] = useState(false);

  useEffect(() => {
    document.documentElement.setAttribute('data-theme', isDark ? 'dark' : 'light');
  }, [isDark]);

  const popoverRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    function handleClickOutside(event: MouseEvent) {
      if (popoverRef.current && !popoverRef.current.contains(event.target as Node)) {
        setIsSearchOpen(false);
        setIsBellOpen(false);
      }
    }
    document.addEventListener('mousedown', handleClickOutside);
    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
    };
  }, []);

  const toggleSearch = () => { setIsSearchOpen(!isSearchOpen); setIsBellOpen(false); };
  const toggleBell = () => { setIsBellOpen(!isBellOpen); setIsSearchOpen(false); };

  return (
    <header className="topbar">
      <div className="topbar-left">
        <div className="topbar-breadcrumb">
          <span>CodeForge</span>
          <span>/</span>
          <span className="current">{currentName}</span>
        </div>
      </div>
      <div className="topbar-right" ref={popoverRef}>
        <div className="popover-container">
          <button className={`btn btn-ghost btn-icon ${isSearchOpen ? 'active' : ''}`} title="搜索" onClick={toggleSearch}>
            <Search size={18} />
          </button>
          {isSearchOpen && (
            <div className="topbar-popover">
              <div className="popover-title">全局搜索</div>
              <div className="popover-content">
                <input type="text" className="popover-input" placeholder="输入关键字、Agent 或文件..." autoFocus />
              </div>
            </div>
          )}
        </div>

        <div className="popover-container">
          <button className={`btn btn-ghost btn-icon ${isBellOpen ? 'active' : ''}`} title="通知" onClick={toggleBell}>
            <Bell size={18} />
          </button>
          {isBellOpen && (
            <div className="topbar-popover">
              <div className="popover-title">
                系统通知
                <span className="badge">3 未读</span>
              </div>
              <div className="popover-content">
                <div className="notification-list">
                  <div className="notification-item">
                    <span><strong>Reviewer</strong> 已完成代码审查任务</span>
                    <span className="notif-time">刚刚</span>
                  </div>
                  <div className="notification-item">
                    <span>您的 <strong>MCP 代理</strong> 已断开连接</span>
                    <span className="notif-time">10 分钟前</span>
                  </div>
                  <div className="notification-item">
                    <span>知识库 <strong>Rust 文档</strong> 索引构建成功</span>
                    <span className="notif-time">2 小时前</span>
                  </div>
                </div>
              </div>
            </div>
          )}
        </div>
        
        <button 
          className={`clever-theme-toggle ${isDark ? 'is-dark' : 'is-light'}`}
          onClick={() => setIsDark(!isDark)}
          title="切换主题"
        >
          <Sun className="icon-sun" size={18} />
          <Moon className="icon-moon" size={18} />
        </button>

        <div className="topbar-status">
          <div className="topbar-status-dot" />
          <span>系统就绪</span>
        </div>
      </div>
    </header>
  );
}
