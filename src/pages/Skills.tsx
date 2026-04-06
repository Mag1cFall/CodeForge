import { Zap, ToggleLeft, ToggleRight, Plus, ChevronDown, ChevronRight } from 'lucide-react';
import { useState, useEffect, useCallback } from 'react';
import { useAppPreferences } from '../lib/app-preferences';
import { skillList as fetchSkillList, skillToggle, SkillRecord } from '../lib/backend';
import './PageCommon.css';

export default function Skills() {
  const { t } = useAppPreferences();
  const [skillList, setSkillList] = useState<SkillRecord[]>([]);
  const [showForm, setShowForm] = useState(false);
  const [form, setForm] = useState({ name: '', path: '' });
  const [expanded, setExpanded] = useState<string | null>(null);

  const loadSkills = useCallback(async () => {
    try {
      const data = await fetchSkillList();
      setSkillList(data ?? []);
    } catch {
      setSkillList([]);
    }
  }, []);

  useEffect(() => {
    void loadSkills();
  }, [loadSkills]);

  const toggle = async (name: string, enabled: boolean) => {
    const nextEnabled = !enabled;
    setSkillList((prev) => prev.map((s) => (s.name === name ? { ...s, enabled: nextEnabled } : s)));
    try {
      await skillToggle(name, nextEnabled);
    } catch {
      await loadSkills();
    }
  };

  const handleInstall = () => {
    if (form.name && form.path) {
      setShowForm(false);
      setForm({ name: '', path: '' });
      void loadSkills();
    }
  };

  const isBuiltinSkill = (path: string) => {
    return path.includes('builtin-skills') || path.includes('\\.system\\') || path.includes('/.system/');
  };

  return (
    <div className="animate-in">
      <div className="page-header">
        <h1><Zap size={28} style={{ verticalAlign: 'middle', marginRight: 8 }} /> {t('route.skills')}</h1>
        <p>{t('page.skills.desc')}</p>
      </div>

      <div className="page-toolbar">
        <button className="btn btn-primary" onClick={() => setShowForm(!showForm)}>
          <Plus size={16} /> {showForm ? '取消' : '安装技能'}
        </button>
      </div>

      {showForm && (
        <div className="card" style={{ marginBottom: 20 }}>
          <div style={{ display: 'grid', gridTemplateColumns: '1fr', gap: 16 }}>
            <div style={{ display: 'flex', gap: 16 }}>
              <div style={{ flex: 1 }}>
                <label>名称</label>
                <input value={form.name} onChange={e => setForm({...form, name: e.target.value})} style={{ width: '100%' }} />
              </div>
              <div style={{ flex: 2 }}>
                <label>SKILL.md 路径或 URL</label>
                <input value={form.path} onChange={e => setForm({...form, path: e.target.value})} placeholder="例如: https://github... 或 C:\path\to\SKILL.md" style={{ width: '100%' }} />
              </div>
            </div>
            <div>
              <button className="btn btn-primary" onClick={handleInstall}>安装</button>
            </div>
          </div>
        </div>
      )}

      <div className="card-grid">
        {skillList.map((skill) => (
          <div key={skill.name} className="card card-glow skill-card" onClick={() => setExpanded(expanded === skill.name ? null : skill.name)} style={{ cursor: 'pointer' }}>
            <div className="skill-card-header">
              <h4 style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
                {expanded === skill.name ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
                {skill.name}
              </h4>
              <button className="btn btn-ghost btn-icon" onClick={(e) => { e.stopPropagation(); void toggle(skill.name, skill.enabled); }}>
                {skill.enabled 
                  ? <ToggleRight size={24} color="var(--accent-green)" />
                  : <ToggleLeft size={24} color="var(--text-tertiary)" />
                }
              </button>
            </div>
            <p className="text-secondary" style={{ fontSize: 13, marginBottom: 12 }}>{skill.description || '暂无描述'}</p>
            <div style={{ display: 'flex', gap: 8 }}>
              {isBuiltinSkill(skill.path)
                ? <span className="badge badge-blue">内置</span>
                : <span className="badge badge-purple">社区</span>
              }
              <span className={`badge badge-${skill.enabled ? 'green' : 'secondary'}`}>{skill.enabled ? '已启用' : '未启用'}</span>
            </div>
            {expanded === skill.name && (
              <div style={{ marginTop: 16, padding: 12, background: 'var(--bg-main)', borderRadius: 6, fontSize: 13, color: 'var(--text-secondary)' }}>
                <strong>Instructions 预览：</strong><br />
                {skill.instructions || '暂无 Instructions'}
              </div>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}
