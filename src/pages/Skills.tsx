import { Zap, Download, Star, ToggleLeft, ToggleRight } from 'lucide-react';
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
  const [skillState, setSkillState] = useState(skills.map(s => s.enabled));

  const toggle = (i: number) => {
    setSkillState(prev => prev.map((v, idx) => idx === i ? !v : v));
  };

  return (
    <div className="animate-in">
      <div className="page-header">
        <h1>⚡ 技能市场</h1>
        <p>Skill = Prompt + Tools + MCP — 赋予 Agent 专业领域能力</p>
      </div>

      <div className="card-grid">
        {skills.map((skill, i) => (
          <div key={skill.name} className="card card-glow skill-card">
            <div className="skill-card-header">
              <h4>{skill.title}</h4>
              <button className="btn btn-ghost btn-icon" onClick={() => toggle(i)}>
                {skillState[i] 
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
              <span className="badge badge-green">{skillState[i] ? '已启用' : '未启用'}</span>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
