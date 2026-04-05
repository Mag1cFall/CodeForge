import { useState } from 'react';
import { Outlet } from 'react-router-dom';
import Sidebar from './Sidebar';
import TopBar from './TopBar';

export default function Layout() {
  const [collapsed, setCollapsed] = useState(false);

  return (
    <div className="layout">
      <Sidebar collapsed={collapsed} onToggle={() => setCollapsed(!collapsed)} />
      <main className={`layout-main ${collapsed ? 'sidebar-collapsed' : ''}`}>
        <TopBar />
        <div className="layout-content">
          <Outlet />
        </div>
      </main>
    </div>
  );
}
