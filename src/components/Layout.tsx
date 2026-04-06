import { useState } from 'react';
import { useLocation } from 'react-router-dom';
import Sidebar from './Sidebar';
import TopBar from './TopBar';

import Dashboard from '../pages/Dashboard';
import Chat from '../pages/Chat';
import Review from '../pages/Review';
import Agents from '../pages/Agents';
import Tools from '../pages/Tools';
import MCP from '../pages/MCP';
import Skills from '../pages/Skills';
import Knowledge from '../pages/Knowledge';
import Providers from '../pages/Providers';
import Logs from '../pages/Logs';
import SettingsPage from '../pages/Settings';

interface KeepAliveProps {
  isActive: boolean;
  children: React.ReactNode;
}

function KeepAlive({ isActive, children }: KeepAliveProps) {
  const [hasMounted, setHasMounted] = useState(isActive);
  if (isActive && !hasMounted) {
    setHasMounted(true);
  }
  
  if (!hasMounted) return null;
  
  // By physically retaining the DOM but removing it from the layout flow,
  // we completely eliminate React rendering and DOM reflow bottlenecks on typical SPAs transitions.
  // The 'page-transition' class ensures a single composite animation on the wrapper.
  return (
    <div className={isActive ? "page-transition" : ""} style={{ display: isActive ? 'block' : 'none', height: '100%', width: '100%' }}>
      {children}
    </div>
  );
}

const pages = [
  { path: '/', component: Dashboard },
  { path: '/chat', component: Chat },
  { path: '/review', component: Review },
  { path: '/agents', component: Agents },
  { path: '/tools', component: Tools },
  { path: '/mcp', component: MCP },
  { path: '/skills', component: Skills },
  { path: '/knowledge', component: Knowledge },
  { path: '/providers', component: Providers },
  { path: '/logs', component: Logs },
  { path: '/settings', component: SettingsPage },
];

export default function Layout() {
  const [collapsed, setCollapsed] = useState(false);
  const location = useLocation();

  return (
    <div className="layout">
      <Sidebar collapsed={collapsed} onToggle={() => setCollapsed(!collapsed)} />
      <main className={`layout-main ${collapsed ? 'sidebar-collapsed' : ''}`}>
        <TopBar />
        <div className="layout-content" style={{ position: 'relative' }}>
          {pages.map(({ path, component: Component }) => {
            const isActive = location.pathname === path || (path === '/' && location.pathname === '');
            return (
              <KeepAlive key={path} isActive={isActive}>
                <Component />
              </KeepAlive>
            );
          })}
        </div>
      </main>
    </div>
  );
}
