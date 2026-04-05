import { BrowserRouter, Routes, Route } from 'react-router-dom';
import Layout from './components/Layout';
import Dashboard from './pages/Dashboard';
import Chat from './pages/Chat';
import Review from './pages/Review';
import Agents from './pages/Agents';
import Tools from './pages/Tools';
import MCP from './pages/MCP';
import Skills from './pages/Skills';
import Knowledge from './pages/Knowledge';
import Providers from './pages/Providers';
import Logs from './pages/Logs';
import SettingsPage from './pages/Settings';

export default function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route element={<Layout />}>
          <Route path="/" element={<Dashboard />} />
          <Route path="/chat" element={<Chat />} />
          <Route path="/review" element={<Review />} />
          <Route path="/agents" element={<Agents />} />
          <Route path="/tools" element={<Tools />} />
          <Route path="/mcp" element={<MCP />} />
          <Route path="/skills" element={<Skills />} />
          <Route path="/knowledge" element={<Knowledge />} />
          <Route path="/providers" element={<Providers />} />
          <Route path="/logs" element={<Logs />} />
          <Route path="/settings" element={<SettingsPage />} />
        </Route>
      </Routes>
    </BrowserRouter>
  );
}
