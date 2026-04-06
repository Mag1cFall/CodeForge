import { Zap, ToggleLeft, ToggleRight, Plus, ChevronDown, ChevronRight } from 'lucide-react';
import { useState } from 'react';
import './PageCommon.css';

const skills = [
  { name: 'code-review', title: '代码审查', desc: '全面的代码质量审查，包括复杂度、命名、错误处理', enabled: true, builtin: true },
  { name: 'best-practices', title: '最佳实践', desc: '语言特定的最佳实践推荐和模式匹配', enabled: true, builtin: true },
  { name: 'security-audit', title: '安全审计', desc: 'OWASP Top 10 安全漏洞检测', enabled: false, builtin: true },
  { name: 'refactoring', title: '代码重构', desc: '智能重构建议，支持提取函数/类/模块', enabled: true, builtin: true },
  { name: 'documentation', title: '文档生成', desc: '自动生成 API 文档和代码注释', enabled: false, builtin: true },
  { name: 'git-master', title: 'Git 大师', desc: '原子化提交、变基手术、冲突解决', enabled: true, builtin: false },
];

export default function Skills() {
  const [skillList, setSkillList] = useState(skills);
  const [showForm, setShowForm] = useState(false);
  const [form, setForm] = useState({ name: '', path: '' });
  const [expanded, setExpanded] = useState<number | null>(null);

  const toggle = (i: number) => {
    setSkillList(prev => prev.map((s, idx) => idx === i ? { ...s, enabled: !s.enabled } : s));
  };

  const handleInstall = () => {
    if (form.name && form.path) {
      setSkillList([...skillList, { name: form.name.toLowerCase().replace(/\s+/g, '-'), title: form.name, desc: `引用于 ${form.path}`, enabled: true, builtin: false }]);
      setShowForm(false);
      setForm({ name: '', path: '' });
    }
  };

  return (
    <div className="animate-in">
      <div className="page-header">
        <h1><Zap size={28} style={{ verticalAlign: 'middle', marginRight: 8 }} /> 技能市场</h1>
        <p>Skill = Prompt + Tools + MCP — 赋予 Agent 专业领域能力</p>
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
        {skillList.map((skill, i) => (
          <div key={skill.name} className="card card-glow skill-card" onClick={() => setExpanded(expanded === i ? null : i)} style={{ cursor: 'pointer' }}>
            <div className="skill-card-header">
              <h4 style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
                {expanded === i ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
                {skill.title}
              </h4>
              <button className="btn btn-ghost btn-icon" onClick={(e) => { e.stopPropagation(); toggle(i); }}>
                {skill.enabled 
                  ? <ToggleRight size={24} color="var(--accent-green)" />
                  : <ToggleLeft size={24} color="var(--text-tertiary)" />
                }
              </button>
            </div>
            <p className="text-secondary" style={{ fontSize: 13, marginBottom: 12 }}>{skill.desc}</p>
            <div style={{ display: 'flex', gap: 8 }}>
              {skill.builtin 
                ? <span className="badge badge-blue">内置</span>
                : <span className="badge badge-purple">社区</span>
              }
              <span className={`badge badge-${skill.enabled ? 'green' : 'secondary'}`}>{skill.enabled ? '已启用' : '未启用'}</span>
            </div>
            {expanded === i && (
              <div style={{ marginTop: 16, padding: 12, background: 'var(--bg-main)', borderRadius: 6, fontSize: 13, color: 'var(--text-secondary)' }}>
                <strong>Instructions 预览：</strong><br />
                {`# ${skill.title}\n${skill.desc}\n\n## 规则\n1. 遵守最佳实践\n2. 优化性能\n3. 编写注释`}
              </div>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}
