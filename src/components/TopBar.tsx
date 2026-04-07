import { useState, useEffect, useRef } from 'react';
import { useLocation, useNavigate } from 'react-router-dom';
import { Bell, Search, Sun, Moon, Minus, Square, X } from 'lucide-react';
import { useAppPreferences } from '../lib/app-preferences';
import { type TranslationKey } from '../lib/i18n';
import { agentList, knowledgeRepos, logList, providerList, sessionList, skillList, toolList } from '../lib/backend';
import { getCurrentWindow } from '@tauri-apps/api/window';
import '../styles/layout.css';

const routeNameKeys: Record<string, TranslationKey> = {
  '/': 'route.dashboard',
  '/chat': 'route.chat',
  '/review': 'route.review',
  '/agents': 'route.agents',
  '/tools': 'route.tools',
  '/mcp': 'route.mcp',
  '/skills': 'route.skills',
  '/knowledge': 'route.knowledge',
  '/providers': 'route.providers',
  '/logs': 'route.logs',
  '/settings': 'route.settings',
};

export default function TopBar() {
  const location = useLocation();
  const navigate = useNavigate();
  const { isDark, setTheme, t } = useAppPreferences();
  const currentName = t(routeNameKeys[location.pathname] || 'topbar.unknown');
  
  const [isSearchOpen, setIsSearchOpen] = useState(false);
  const [isBellOpen, setIsBellOpen] = useState(false);
  const [searchQuery, setSearchQuery] = useState('');
  const [searchResults, setSearchResults] = useState<Array<{ id: string; label: string; meta: string; path: string }>>([]);
  const [searchLoading, setSearchLoading] = useState(false);
  const [notifications, setNotifications] = useState<Array<{ id: string; title: string; time: string }>>([]);
  const [unreadCount, setUnreadCount] = useState(0);

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

  useEffect(() => {
    if (!isSearchOpen) {
      return;
    }

    const keyword = searchQuery.trim().toLowerCase();
    if (!keyword) {
      setSearchResults(
        Object.entries(routeNameKeys).map(([path, key]) => ({
          id: path,
          label: t(key),
          meta: path,
          path,
        }))
      );
      setSearchLoading(false);
      return;
    }

    let cancelled = false;
    setSearchLoading(true);
    void (async () => {
      const [agents, providers, tools, skills, sessions, repos] = await Promise.all([
        agentList().catch(() => []),
        providerList().catch(() => []),
        toolList().catch(() => []),
        skillList().catch(() => []),
        sessionList().catch(() => []),
        knowledgeRepos().catch(() => []),
      ]);

      const matches = [
        ...Object.entries(routeNameKeys).map(([path, key]) => ({ id: `route-${path}`, label: t(key), meta: path, path })),
        ...agents.map((item) => ({ id: `agent-${item.id}`, label: item.name, meta: t('route.agents'), path: '/agents' })),
        ...providers.map((item) => ({ id: `provider-${item.id}`, label: item.name, meta: t('route.providers'), path: '/providers' })),
        ...tools.map((item) => ({ id: `tool-${item.name}`, label: item.name, meta: t('route.tools'), path: '/tools' })),
        ...skills.map((item) => ({ id: `skill-${item.id}`, label: item.name, meta: t('route.skills'), path: '/skills' })),
        ...sessions.map((item) => ({ id: `session-${item.id}`, label: item.title, meta: t('route.chat'), path: '/chat' })),
        ...repos.map((item) => ({ id: `repo-${item.id}`, label: item.path, meta: t('route.knowledge'), path: '/knowledge' })),
      ].filter((item) => `${item.label} ${item.meta}`.toLowerCase().includes(keyword));

      if (!cancelled) {
        setSearchResults(matches.slice(0, 12));
        setSearchLoading(false);
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [isSearchOpen, searchQuery, t]);

  useEffect(() => {
    if (!isBellOpen) {
      return;
    }

    void (async () => {
      const logs = await logList(5).catch(() => []);
      const next = logs.map((item) => {
        const payload = item.payload;
        const action = typeof payload.action === 'string'
          ? payload.action
          : typeof payload.name === 'string'
            ? payload.name
            : item.kind;
        return {
          id: String(item.id),
          title: `${item.kind} · ${action}`,
          time: new Date(item.createdAt).toLocaleString('zh-CN', { hour: '2-digit', minute: '2-digit' }),
        };
      });
      setNotifications(next);
      setUnreadCount(0);
    })();
  }, [isBellOpen]);

  useEffect(() => {
    void (async () => {
      const logs = await logList(3).catch(() => []);
      setUnreadCount(logs.length);
    })();
  }, []);
  const handleSearchNavigate = (path: string) => {
    navigate(path);
    setIsSearchOpen(false);
  };

  const win = getCurrentWindow();

  return (
    <header className="topbar">
      <div className="topbar-left" style={{ zIndex: 10 }}>
        <div className="topbar-breadcrumb">
          <span>CodeForge</span>
          <span>/</span>
          <span className="current">{currentName}</span>
        </div>
      </div>
      
      {/* 专用无干扰全局拖拽区 */}
      <div data-tauri-drag-region className="topbar-drag-area" />

      <div className="topbar-right" ref={popoverRef} style={{ zIndex: 10 }}>
        <div className="popover-container">
          <button type="button" className={`btn btn-ghost btn-icon ${isSearchOpen ? 'active' : ''}`} title={t('topbar.search')} onClick={toggleSearch}>
            <Search size={18} />
          </button>
          {isSearchOpen && (
            <div className="topbar-popover">
              <div className="popover-title">{t('topbar.searchTitle')}</div>
              <div className="popover-content">
                <input type="text" className="popover-input" placeholder="输入关键字、Agent 或文件..." value={searchQuery} onChange={(event) => setSearchQuery(event.target.value)} />
                <div className="notification-list">
                  {searchLoading && <div className="notification-item"><span>搜索中...</span></div>}
                  {!searchLoading && searchResults.map((item) => (
                    <button key={item.id} type="button" className="notification-item" onClick={() => handleSearchNavigate(item.path)}>
                      <span><strong>{item.label}</strong> · {item.meta}</span>
                    </button>
                  ))}
                  {!searchLoading && searchResults.length === 0 && <div className="notification-item"><span>没有搜索结果</span></div>}
                </div>
              </div>
            </div>
          )}
        </div>

        <div className="popover-container">
          <button type="button" className={`btn btn-ghost btn-icon ${isBellOpen ? 'active' : ''}`} title={t('topbar.notifications')} onClick={toggleBell}>
            <Bell size={18} />
          </button>
          {isBellOpen && (
            <div className="topbar-popover">
              <div className="popover-title">
                系统通知
                <span className="badge">{unreadCount} 未读</span>
              </div>
              <div className="popover-content">
                <div className="notification-list">
                  {notifications.map((item) => (
                    <button key={item.id} type="button" className="notification-item" onClick={() => navigate('/logs')}>
                      <span>{item.title}</span>
                      <span className="notif-time">{item.time}</span>
                    </button>
                  ))}
                  {notifications.length === 0 && (
                    <div className="notification-item">
                      <span>暂无通知</span>
                    </div>
                  )}
                </div>
              </div>
            </div>
          )}
        </div>
        
        <button
          type="button"
          className={`clever-theme-toggle ${isDark ? 'is-dark' : 'is-light'}`}
          onClick={(e) => setTheme(isDark ? 'light' : 'dark', e)}
          title={t('topbar.themeToggle')}
        >
          <Sun className="icon-sun" size={18} />
          <Moon className="icon-moon" size={18} />
        </button>

        <div className="window-controls">
          <button className="window-btn" onClick={() => void win.minimize()}><Minus size={14} /></button>
          <button className="window-btn" onClick={() => void win.toggleMaximize()}><Square size={12} /></button>
          <button className="window-btn window-btn-close" onClick={() => void win.close()}><X size={16} /></button>
        </div>

      </div>
    </header>
  );
}
