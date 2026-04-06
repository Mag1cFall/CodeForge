import { NavLink } from 'react-router-dom';
import {
  LayoutDashboard, MessageSquare, SearchCode, Bot,
  Wrench, Plug, Zap, BookOpen, Server, ScrollText,
  Settings, PanelLeftClose, PanelLeft, Flame
} from 'lucide-react';
import { useAppPreferences } from '../lib/app-preferences';
import { type TranslationKey } from '../lib/i18n';
import '../styles/sidebar.css';

interface NavSection {
  labelKey: TranslationKey;
  items: Array<{ path: string; icon: typeof LayoutDashboard; labelKey: TranslationKey }>;
}

const navSections: NavSection[] = [
  {
    labelKey: 'sidebar.section.core',
    items: [
      { path: '/', icon: LayoutDashboard, labelKey: 'route.dashboard' },
      { path: '/chat', icon: MessageSquare, labelKey: 'route.chat' },
      { path: '/review', icon: SearchCode, labelKey: 'route.review' },
    ]
  },
  {
    labelKey: 'sidebar.section.framework',
    items: [
      { path: '/agents', icon: Bot, labelKey: 'route.agents' },
      { path: '/tools', icon: Wrench, labelKey: 'route.tools' },
      { path: '/mcp', icon: Plug, labelKey: 'route.mcp' },
      { path: '/skills', icon: Zap, labelKey: 'route.skills' },
    ]
  },
  {
    labelKey: 'sidebar.section.data',
    items: [
      { path: '/knowledge', icon: BookOpen, labelKey: 'route.knowledge' },
      { path: '/providers', icon: Server, labelKey: 'route.providers' },
      { path: '/logs', icon: ScrollText, labelKey: 'route.logs' },
    ]
  },
  {
    labelKey: 'sidebar.section.system',
    items: [
      { path: '/settings', icon: Settings, labelKey: 'route.settings' },
    ]
  }
];

interface SidebarProps {
  collapsed: boolean;
  onToggle: () => void;
}

export default function Sidebar({ collapsed, onToggle }: SidebarProps) {
  const { t } = useAppPreferences();

  return (
    <aside className={`sidebar ${collapsed ? 'collapsed' : ''}`}>
      <div className="sidebar-header">
        <div className="sidebar-logo">
          <Flame size={18} />
        </div>
        <div className="sidebar-brand">
          <h2>CodeForge</h2>
          <span>炼码 · AI Coding Agent</span>
        </div>
      </div>

      <nav className="sidebar-nav">
        {navSections.map((section) => (
          <div key={section.labelKey}>
            <div className="nav-section-label">{t(section.labelKey)}</div>
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
                <span className="nav-item-label">{t(item.labelKey)}</span>
              </NavLink>
            ))}
          </div>
        ))}
      </nav>

      <div className="sidebar-footer">
        <button type="button" className="sidebar-toggle" onClick={onToggle}>
          {collapsed ? <PanelLeft size={18} /> : <PanelLeftClose size={18} />}
          {!collapsed && <span>{t('sidebar.toggle')}</span>}
        </button>
      </div>
    </aside>
  );
}
