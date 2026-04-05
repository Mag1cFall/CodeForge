import { Settings as SettingsIcon, Moon, Globe, FolderOpen, Shield, Save } from 'lucide-react';
import './PageCommon.css';

export default function SettingsPage() {
  return (
    <div className="animate-in">
      <div className="page-header">
        <h1>⚙️ 设置</h1>
        <p>全局配置、主题、语言、项目路径</p>
      </div>

      <div style={{ display: 'flex', flexDirection: 'column', gap: 20 }}>
        <div className="card">
          <h3 style={{ fontSize: 15, fontWeight: 600, marginBottom: 20, display: 'flex', alignItems: 'center', gap: 8 }}>
            <Moon size={18} /> 外观
          </h3>
          <div className="settings-row">
            <label>主题</label>
            <select defaultValue="dark">
              <option value="dark">深色</option>
              <option value="light">浅色</option>
              <option value="auto">跟随系统</option>
            </select>
          </div>
          <div className="settings-row">
            <label>语言</label>
            <select defaultValue="zh">
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
            <input type="text" defaultValue="C:\Users\l\Desktop\数据挖掘\codeforge" style={{ flex: 1 }} />
          </div>
          <div className="settings-row">
            <label>Skills 目录</label>
            <input type="text" defaultValue="~/.codeforge/skills" style={{ flex: 1 }} />
          </div>
        </div>

        <div className="card">
          <h3 style={{ fontSize: 15, fontWeight: 600, marginBottom: 20, display: 'flex', alignItems: 'center', gap: 8 }}>
            <Shield size={18} /> Harness 安全
          </h3>
          <div className="settings-row">
            <label>Shell 命令确认</label>
            <select defaultValue="ask">
              <option value="ask">每次询问</option>
              <option value="auto">自动执行（危险）</option>
              <option value="deny">全部拒绝</option>
            </select>
          </div>
          <div className="settings-row">
            <label>Token 预算 (每次会话)</label>
            <input type="number" defaultValue={100000} style={{ width: 150 }} />
          </div>
        </div>

        <button className="btn btn-primary" style={{ alignSelf: 'flex-start' }}>
          <Save size={16} /> 保存设置
        </button>
      </div>
    </div>
  );
}
