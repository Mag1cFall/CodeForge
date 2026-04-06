import { useState, useEffect, useCallback } from 'react';
import { Moon, FolderOpen, Shield, Save, Settings2, CheckCircle2 } from 'lucide-react';
import { useAppPreferences } from '../lib/app-preferences';
import { normalizeTheme } from '../lib/i18n';
import { settingsGet, settingsUpdate } from '../lib/backend';
import './PageCommon.css';

export default function SettingsPage() {
  const { theme, setTheme, locale, setLocale, t } = useAppPreferences();
  const [projectPath, setProjectPath] = useState('');
  const [skillsPath, setSkillsPath] = useState('~/.codeforge/skills');
  const [shellConfirm, setShellConfirm] = useState('agent');
  const [tokenBudget, setTokenBudget] = useState('10000000');
  const [contextWindowOverridesText, setContextWindowOverridesText] = useState('{}');
  const [saved, setSaved] = useState(false);

  const loadSettings = useCallback(async () => {
    let loadedSkillsPath = '~/.skills';
    try {
      const data = await settingsGet();
      setTheme(data.theme === 'light' || data.theme === 'auto' ? data.theme : 'dark');
      setLocale(data.language === 'zh-TW' || data.language === 'en-US' ? (data.language === 'en-US' ? 'en' : 'zh-TW') : 'zh-CN');
      setProjectPath(data.projectPath || '');
      loadedSkillsPath = data.skillsPath || loadedSkillsPath;
      setContextWindowOverridesText(JSON.stringify(data.contextWindowOverrides || {}, null, 2));
    } catch {
      setTheme('dark');
      setLocale('zh-CN');
      setProjectPath('');
      setContextWindowOverridesText('{}');
    }

    setSkillsPath(localStorage.getItem('skillsPath') || loadedSkillsPath);
    setShellConfirm(localStorage.getItem('shellConfirm') || 'agent');
    setTokenBudget(localStorage.getItem('tokenBudget') || '10000000');
  }, [setLocale, setTheme]);

  useEffect(() => {
    void loadSettings();
  }, [loadSettings]);

  const handleSave = () => {
    void (async () => {
      let contextWindowOverrides: Record<string, number> = {};
      try {
        const parsed = JSON.parse(contextWindowOverridesText) as Record<string, unknown>;
        contextWindowOverrides = Object.fromEntries(
          Object.entries(parsed).filter((entry): entry is [string, number] => typeof entry[1] === 'number' && entry[1] > 0)
        );
      } catch {
        return;
      }

      try {
        await settingsUpdate({
          theme,
          language: locale === 'en' ? 'en-US' : locale,
          projectPath: projectPath.trim() ? projectPath : null,
          contextWindowOverrides,
        });
      } catch {
      }

      localStorage.setItem('skillsPath', skillsPath);
      localStorage.setItem('shellConfirm', shellConfirm);
      localStorage.setItem('tokenBudget', tokenBudget);

      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    })();
  };

  return (
    <div className="animate-in">
      <div className="page-header">
        <h1><Settings2 size={28} style={{ verticalAlign: 'middle', marginRight: 8 }} /> {t('route.settings')}</h1>
        <p>{t('page.settings.desc')}</p>
      </div>

      <div style={{ display: 'flex', flexDirection: 'column', gap: 20 }}>
        <div className="card">
          <h3 style={{ fontSize: 15, fontWeight: 600, marginBottom: 20, display: 'flex', alignItems: 'center', gap: 8 }}>
            <Moon size={18} /> 外观
          </h3>
          <div className="settings-row">
            <label>{t('settings.theme')}</label>
            <select value={theme} onChange={e => setTheme(normalizeTheme(e.target.value))}>
              <option value="dark">{t('settings.theme.dark')}</option>
              <option value="light">{t('settings.theme.light')}</option>
              <option value="auto">{t('settings.theme.auto')}</option>
            </select>
          </div>
          <div className="settings-row">
            <label>{t('settings.language')}</label>
            <select value={locale} onChange={e => setLocale(e.target.value === 'zh-TW' || e.target.value === 'en' ? e.target.value : 'zh-CN')}>
              <option value="zh-CN">{t('settings.language.zh-CN')}</option>
              <option value="zh-TW">{t('settings.language.zh-TW')}</option>
              <option value="en">{t('settings.language.en')}</option>
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
          <div className="settings-row" style={{ alignItems: 'flex-start' }}>
            <label>上下文窗口覆盖</label>
            <textarea
              value={contextWindowOverridesText}
              onChange={e => setContextWindowOverridesText(e.target.value)}
              style={{ flex: 1, minHeight: 120, fontFamily: 'var(--font-mono)' }}
              placeholder={'{\n  "gpt-5.4-mini": 400000,\n  "openaicompatible/gpt-5.4": 1000000\n}'}
            />
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
