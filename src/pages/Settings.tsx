import { useState } from 'react';
import { Moon, FolderOpen, Shield, Save, Settings2, CheckCircle2 } from 'lucide-react';
import './PageCommon.css';

export default function SettingsPage() {
  const [theme, setTheme] = useState(localStorage.getItem('theme') || 'dark');
  const [lang, setLang] = useState(localStorage.getItem('lang') || 'zh');
  const [projectPath, setProjectPath] = useState(localStorage.getItem('projectPath') || 'C:\\Users\\l\\Desktop\\数据挖掘\\codeforge');
  const [skillsPath, setSkillsPath] = useState(localStorage.getItem('skillsPath') || '~/.codeforge/skills');
  const [shellConfirm, setShellConfirm] = useState(localStorage.getItem('shellConfirm') || 'agent');
  const [tokenBudget, setTokenBudget] = useState(localStorage.getItem('tokenBudget') || '10000000');
  const [saved, setSaved] = useState(false);

  const handleSave = () => {
    localStorage.setItem('theme', theme);
    localStorage.setItem('lang', lang);
    localStorage.setItem('projectPath', projectPath);
    localStorage.setItem('skillsPath', skillsPath);
    localStorage.setItem('shellConfirm', shellConfirm);
    localStorage.setItem('tokenBudget', tokenBudget);
    
    setSaved(true);
    setTimeout(() => setSaved(false), 2000);
  };

  return (
    <div className="animate-in">
      <div className="page-header">
        <h1><Settings2 size={28} style={{ verticalAlign: 'middle', marginRight: 8 }} /> 设置</h1>
        <p>全局配置、主题、语言、项目路径</p>
      </div>

      <div style={{ display: 'flex', flexDirection: 'column', gap: 20 }}>
        <div className="card">
          <h3 style={{ fontSize: 15, fontWeight: 600, marginBottom: 20, display: 'flex', alignItems: 'center', gap: 8 }}>
            <Moon size={18} /> 外观
          </h3>
          <div className="settings-row">
            <label>主题</label>
            <select value={theme} onChange={e => setTheme(e.target.value)}>
              <option value="dark">深色</option>
              <option value="light">浅色</option>
              <option value="auto">跟随系统</option>
            </select>
          </div>
          <div className="settings-row">
            <label>语言</label>
            <select value={lang} onChange={e => setLang(e.target.value)}>
              <option value="zh">简体中文</option>
              <option value="en">English</option>
            </select>
          </div>
        </div>

        <div className="card">
          <h3 style={{ fontSize: 15, fontWeight: 600, marginBottom: 20, display: 'flex', alignItems: 'center', gap: 8 }}>
            <FolderOpen size={18} /> 项目
          </h3>
          <div className="settings-row">
            <label>默认项目路径</label>
            <input type="text" value={projectPath} onChange={e => setProjectPath(e.target.value)} style={{ flex: 1 }} />
          </div>
          <div className="settings-row">
            <label>Skills 目录</label>
            <input type="text" value={skillsPath} onChange={e => setSkillsPath(e.target.value)} style={{ flex: 1 }} />
          </div>
        </div>

        <div className="card">
          <h3 style={{ fontSize: 15, fontWeight: 600, marginBottom: 20, display: 'flex', alignItems: 'center', gap: 8 }}>
            <Shield size={18} /> Harness 执行约束
          </h3>
          <div className="settings-row">
            <label>Shell 命令确认</label>
            <select value={shellConfirm} onChange={e => setShellConfirm(e.target.value)}>
              <option value="ask">每次询问</option>
              <option value="agent">Agent 自主判断</option>
              <option value="auto">自动执行（危险）</option>
              <option value="deny">全部拒绝</option>
            </select>
          </div>
          <div className="settings-row">
            <label>Token 预算 (每次会话)</label>
            <input type="number" value={tokenBudget} onChange={e => setTokenBudget(e.target.value)} style={{ width: 180 }} />
          </div>
        </div>

        <div style={{ display: 'flex', alignItems: 'center', gap: 16 }}>
          <button className="btn btn-primary" onClick={handleSave}>
            <Save size={16} /> 保存设置
          </button>
          {saved && <span style={{ color: 'var(--accent-green-light)', display: 'flex', alignItems: 'center', gap: 4 }}><CheckCircle2 size={16} /> 已保存</span>}
        </div>
      </div>
    </div>
  );
}
