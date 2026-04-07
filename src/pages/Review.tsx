import { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  SearchCode, FolderOpen, GitBranch, Play,
  AlertTriangle, CheckCircle2, XCircle, ChevronRight,
  Shield, Box, FileCode, RotateCcw,
  Download, ClipboardCopy, Settings2, Zap, Brain, Filter
} from 'lucide-react';
import {
  projectOpen,
  projectClone,
  projectReview,
  projectReviewAi,
  listenReviewProgress,
  listenReviewResult,
  ReviewIssue,
  skillList,
  skillToggle,
  SkillRecord,
  agentList,
  AgentRecord,
} from '../lib/backend';
import { useAppPreferences } from '../lib/app-preferences';
import './Review.css';

type Phase = 'select' | 'configure' | 'running' | 'results';
type ReviewMode = 'fast' | 'ai';
type SeverityFilter = 'all' | 'error' | 'warning' | 'info';

interface RunningLogItem {
  time: string;
  msg: string;
}

const nowTime = (): string => {
  return new Date().toLocaleTimeString('zh-CN', { hour12: false });
};

export default function Review() {
  const { t } = useAppPreferences();
  const navigate = useNavigate();
  const [phase, setPhase] = useState<Phase>('select');
  const [projectPath, setProjectPath] = useState('');
  const [gitUrl, setGitUrl] = useState('');
  const [selectedAgent, setSelectedAgent] = useState('Reviewer');
  const [sandboxEnabled, setSandboxEnabled] = useState(true);
  const [runningLog, setRunningLog] = useState<RunningLogItem[]>([]);
  const [issues, setIssues] = useState<ReviewIssue[]>([]);
  const [selectedIssue, setSelectedIssue] = useState<number | null>(null);
  const [scannedFiles, setScannedFiles] = useState(0);
  const [skills, setSkills] = useState<SkillRecord[]>([]);
  const [agents, setAgents] = useState<AgentRecord[]>([]);
  const [showSkillModal, setShowSkillModal] = useState(false);
  const [skillSearchText, setSkillSearchText] = useState('');
  const [reviewMode, setReviewMode] = useState<ReviewMode>('ai');
  const [reviewScope, setReviewScope] = useState('all');
  const [severityFilter, setSeverityFilter] = useState<SeverityFilter>('all');
  const [progressPercent, setProgressPercent] = useState(0);
  const [totalBatches, setTotalBatches] = useState(0);
  const [completedBatches, setCompletedBatches] = useState(0);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  useEffect(() => {
    let unlistenProgress: (() => void) | null = null;
    let unlistenResult: (() => void) | null = null;

    const subscribe = async () => {
      try {
        unlistenProgress = await listenReviewProgress((payload) => {
          setRunningLog((prev) => [...prev, { time: nowTime(), msg: payload.log }]);

          if (payload.step === 'scan') {
            const fileMatch = payload.log.match(/共\s*(\d+)\s*个文件/);
            if (fileMatch) {
              const total = parseInt(fileMatch[1], 10);
              setScannedFiles(total);
              setTotalBatches(Math.ceil(total / 8));
            }
            setProgressPercent(10);
          } else if (payload.step === 'review') {
            const batchMatch = payload.log.match(/(\d+)\/(\d+)\s*批/);
            if (batchMatch) {
              const current = parseInt(batchMatch[1], 10);
              const total = parseInt(batchMatch[2], 10);
              setCompletedBatches(current);
              setTotalBatches(total);
              setProgressPercent(10 + Math.round((current / total) * 75));
            }
          } else if (payload.step === 'heuristic') {
            setProgressPercent(90);
          } else if (payload.step === 'complete') {
            setProgressPercent(100);
          }
        });
      } catch {
        unlistenProgress = null;
      }

      try {
        unlistenResult = await listenReviewResult((payload) => {
          setIssues(payload ?? []);
          setProgressPercent(100);
          setPhase('results');
        });
      } catch {
        unlistenResult = null;
      }
    };

    const fetchSkills = async () => {
      try {
        const REVIEW_SKILLS = ['code-review', 'best-practices', 'security-audit', 'refactoring', 'documentation'];
        const data = await skillList();
        if (!data) return;

        const updatedData = [...data];
        for (const s of updatedData) {
          const isReviewSkill = REVIEW_SKILLS.includes(s.name);
          if (s.enabled !== isReviewSkill) {
            try {
              await skillToggle(s.name, isReviewSkill);
              s.enabled = isReviewSkill;
            } catch { /* skill toggle best-effort */ }
          }
        }
        setSkills(updatedData);
      } catch {
        setSkills([]);
      }
    };

    const fetchAgents = async () => {
      try {
        const data = await agentList();
        if (data && data.length > 0) {
          setAgents(data);
          const reviewer = data.find(a => a.name.toLowerCase().includes('reviewer'));
          setSelectedAgent(reviewer ? reviewer.name : data[0].name);
        }
      } catch {
        setAgents([]);
      }
    };

    void fetchSkills();
    void fetchAgents();
    void subscribe();
    return () => {
      if (unlistenProgress) unlistenProgress();
      if (unlistenResult) unlistenResult();
    };
  }, []);

  const startReview = () => {
    void (async () => {
      setPhase('running');
      setRunningLog([]);
      setIssues([]);
      setSelectedIssue(null);
      setProgressPercent(0);
      setCompletedBatches(0);
      setTotalBatches(0);

      setErrorMessage(null);

      try {
        let reviewPath = projectPath.trim();

        if (gitUrl.trim()) {
          const cloned = await projectClone(gitUrl.trim());
          reviewPath = cloned.path;
          setProjectPath(cloned.path);
        }

        if (!reviewPath) {
          setPhase('configure');
          return;
        }

        try {
          const info = await projectOpen(reviewPath);
          setScannedFiles(info.fileCount);
        } catch {
          setScannedFiles(0);
        }

        if (reviewMode === 'ai') {
          await projectReviewAi({
            path: reviewPath,
            sandbox: sandboxEnabled,
            agentName: selectedAgent,
            scope: reviewScope,
          });
        } else {
          await projectReview(reviewPath, sandboxEnabled);
        }
        setPhase('results');
      } catch (error) {
        console.error('Review failed:', error);
        setErrorMessage(typeof error === 'string' ? error : (error as Error).message);
        setIssues([]);
        setPhase('results');
      }
    })();
  };

  const filteredIssues = severityFilter === 'all'
    ? issues
    : issues.filter((issue) => {
        if (severityFilter === 'info') return issue.severity !== 'error' && issue.severity !== 'warning';
        return issue.severity === severityFilter;
      });

  const errorCount = issues.filter((issue) => issue.severity === 'error').length;
  const warningCount = issues.filter((issue) => issue.severity === 'warning').length;
  const infoCount = issues.filter((issue) => issue.severity !== 'error' && issue.severity !== 'warning').length;
  const currentIssue = selectedIssue !== null ? filteredIssues[selectedIssue] : null;

  const handleCopyReport = async () => {
    const lines = issues.map(
      (issue) => `[${issue.severity.toUpperCase()}] ${issue.file}:${issue.line} — ${issue.rule}\n  ${issue.message}\n  建议: ${issue.suggestion}`
    );
    const report = `CodeForge 审查报告 (${new Date().toLocaleString('zh-CN')})\n${'='.repeat(60)}\n共 ${issues.length} 个问题 (${errorCount} 错误, ${warningCount} 警告, ${infoCount} 建议)\n\n${lines.join('\n\n')}`;
    try {
      await navigator.clipboard.writeText(report);
    } catch { /* clipboard not available */ }
  };

  const handleExport = () => {
    const blob = new Blob([JSON.stringify(issues, null, 2)], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `codeforge-review-${Date.now()}.json`;
    a.click();
    URL.revokeObjectURL(url);
  };

  const handleAutofix = () => {
    const summary = issues
      .filter((i) => i.severity === 'error' || i.severity === 'warning')
      .slice(0, 20)
      .map((i) => `- [${i.severity}] ${i.file}:${i.line} — ${i.message}\n  建议: ${i.suggestion}`)
      .join('\n');
    const prompt = `以下是代码审查发现的问题，请帮我逐一修复：\n\n项目路径: ${projectPath}\n\n${summary}\n\n请读取相关文件并修复这些问题。`;
    sessionStorage.setItem('codeforge_autofix_prompt', prompt);
    navigate('/chat?autofix=1');
  };

  return (
    <div className="review-page animate-in">
      <div className="page-header">
        <h1><SearchCode size={28} style={{ verticalAlign: 'middle', marginRight: 8 }} /> {t('route.review')}</h1>
        <p>{t('page.review.desc')}</p>
      </div>

      <div className="review-phases">
        {['select', 'configure', 'running', 'results'].map((p, i) => (
          <div key={p} className={`review-phase-dot ${phase === p ? 'active' : ''} ${['select', 'configure', 'running', 'results'].indexOf(phase) > i ? 'done' : ''}`}>
            <span>{i + 1}</span>
            <label>{['选择项目', '配置审查', '沙箱执行', '审查报告'][i]}</label>
          </div>
        ))}
      </div>

      {phase === 'select' && (
        <div className="review-select">
          <div className="card review-option" onClick={async () => {
            try {
              const { open } = await import('@tauri-apps/plugin-dialog');
              const selected = await open({ directory: true });
              if (selected && typeof selected === 'string') {
                setProjectPath(selected);
                setGitUrl('');
              }
            } catch { /* dialog cancelled */ }
          }}>
            <FolderOpen size={32} color="var(--accent-blue-light)" />
            <h3>本地文件夹</h3>
            <p>选择本地项目目录进行审查</p>
            <div style={{ display: 'flex', gap: 8 }} onClick={e => e.stopPropagation()}>
              <input
                placeholder="项目路径..."
                value={projectPath}
                onChange={e => {
                  setProjectPath(e.target.value);
                  setGitUrl('');
                }}
                style={{ flex: 1 }}
              />
              <button
                className="btn btn-primary"
                onClick={() => { if (projectPath) setPhase('configure'); }}
                disabled={!projectPath}
              >
                下一步
              </button>
            </div>
          </div>
          <div className="card review-option">
            <GitBranch size={32} color="var(--accent-purple-light)" />
            <h3>Git 仓库</h3>
            <p>Agent 自主克隆仓库到沙箱审查</p>
            <div style={{ display: 'flex', gap: 8 }}>
              <input
                placeholder="https://github.com/user/repo"
                value={gitUrl}
                onChange={e => {
                  setGitUrl(e.target.value);
                  setProjectPath('');
                }}
                style={{ flex: 1 }}
              />
              <button
                className="btn btn-primary"
                onClick={() => { if (gitUrl) setPhase('configure'); }}
                disabled={!gitUrl}
              >
                下一步
              </button>
            </div>
          </div>
        </div>
      )}

      {phase === 'configure' && (
        <div className="review-configure">
          <div className="card">
            <h3><Settings2 size={18} /> 审查配置</h3>
            <div className="config-grid">
              <div className="config-item">
                <label>审查模式</label>
                <div style={{ display: 'flex', gap: 8 }}>
                  <button
                    type="button"
                    className={`btn ${reviewMode === 'ai' ? 'btn-primary' : 'btn-secondary'}`}
                    onClick={() => setReviewMode('ai')}
                    style={{ flex: 1, display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 6 }}
                  >
                    <Brain size={14} /> AI 深度分析
                  </button>
                  <button
                    type="button"
                    className={`btn ${reviewMode === 'fast' ? 'btn-primary' : 'btn-secondary'}`}
                    onClick={() => setReviewMode('fast')}
                    style={{ flex: 1, display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 6 }}
                  >
                    <Zap size={14} /> 快速扫描
                  </button>
                </div>
                <span className="config-hint">
                  {reviewMode === 'ai'
                    ? 'Agent 逐文件分析，发现深层问题（较慢但精准）'
                    : '正则 + 规则匹配，秒级完成（快速但浅层）'}
                </span>
              </div>

              {reviewMode === 'ai' && (
                <>
                  <div className="config-item">
                    <label>Agent</label>
                    <select value={selectedAgent} onChange={e => setSelectedAgent(e.target.value)}>
                      {agents.length > 0 ? agents.map(a => (
                        <option key={a.id} value={a.name}>{a.name}</option>
                      )) : (
                        <option>Reviewer</option>
                      )}
                    </select>
                  </div>
                  <div className="config-item">
                    <label>审查范围</label>
                    <select value={reviewScope} onChange={e => setReviewScope(e.target.value)}>
                      <option value="all">全部文件</option>
                      <option value="changed">仅修改文件 (git diff)</option>
                      <option value="src">仅 src/ 目录</option>
                    </select>
                  </div>
                </>
              )}

              <div className="config-item">
                <label>
                  <Shield size={14} /> 沙箱隔离
                </label>
                <div className="toggle-row">
                  <button
                    type="button"
                    className={`toggle-btn ${sandboxEnabled ? 'on' : ''}`}
                    onClick={() => setSandboxEnabled(!sandboxEnabled)}
                  >
                    <span className="toggle-knob" />
                  </button>
                  <span style={{ fontSize: 13, fontWeight: 500, color: sandboxEnabled ? 'var(--text-primary)' : 'var(--text-secondary)' }}>
                    {sandboxEnabled ? '启用' : '禁用'}
                  </span>
                  <span className="config-hint">
                    {sandboxEnabled ? '项目目录只读挂载，所有修改在隔离环境执行' : '⚠️ Agent 可直接修改文件'}
                  </span>
                </div>
              </div>
              <div className="config-item">
                <label>启用的 Skill</label>
                <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap', alignItems: 'center' }}>
                  {skills.filter(s => s.enabled).map(s => (
                    <span key={s.id} className="badge badge-blue" title={s.description}>
                      {s.name}
                    </span>
                  ))}
                  {skills.filter(s => s.enabled).length === 0 && <span className="text-secondary" style={{ fontSize: 13 }}>未启用附加 Skill</span>}
                  <button 
                    type="button" 
                    className="btn btn-sm btn-ghost" 
                    onClick={() => setShowSkillModal(true)}
                    style={{ padding: '2px 8px', fontSize: 13 }}
                  >
                    + 管理技能
                  </button>
                </div>
              </div>
            </div>
            <div className="config-actions">
              <button className="btn btn-secondary" onClick={() => setPhase('select')}>返回</button>
              <button className="btn btn-primary" onClick={startReview}>
                <Play size={16} /> {reviewMode === 'ai' ? 'AI 审查' : '快速扫描'}
              </button>
            </div>
          </div>
          
          {showSkillModal && (
            <div style={{ position: 'fixed', inset: 0, zIndex: 100, background: 'rgba(0,0,0,0.5)', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
              <div className="card" style={{ width: 500, maxHeight: '80vh', display: 'flex', flexDirection: 'column', padding: '24px' }}>
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 16 }}>
                  <h3 style={{ margin: 0, fontSize: 16, fontWeight: 500 }}>管理启动项 Skill</h3>
                  <button className="btn btn-ghost btn-icon btn-sm" onClick={() => setShowSkillModal(false)}><XCircle size={16} /></button>
                </div>
                
                <input 
                  type="text" 
                  placeholder="搜索 Skill..." 
                  value={skillSearchText} 
                  onChange={e => setSkillSearchText(e.target.value)}
                  style={{ width: '100%', padding: '8px 12px', marginBottom: 16, background: 'var(--bg-input)', border: '1px solid var(--border-primary)', borderRadius: 4, color: 'var(--text-primary)', outline: 'none' }}
                />
                
                <div style={{ flex: 1, overflowY: 'auto', display: 'flex', flexDirection: 'column', gap: 8 }}>
                  {skills.filter(s => s.name.toLowerCase().includes(skillSearchText.toLowerCase()) || s.description.toLowerCase().includes(skillSearchText.toLowerCase())).map(s => (
                    <div key={s.id} className="issue-row" style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', cursor: 'default' }}>
                      <div style={{ display: 'flex', flexDirection: 'column', gap: 4, flex: 1, paddingRight: 16 }}>
                        <span style={{ fontSize: 14, fontWeight: 500 }}>{s.name}</span>
                        <span style={{ fontSize: 12, color: 'var(--text-secondary)' }}>{s.description}</span>
                      </div>
                      <button
                        type="button"
                        className={`toggle-btn ${s.enabled ? 'on' : ''}`}
                        onClick={async () => {
                          const targetEnabled = !s.enabled;
                          try {
                            await skillToggle(s.name, targetEnabled);
                            setSkills(prev => prev.map(item => item.id === s.id ? { ...item, enabled: targetEnabled } : item));
                          } catch { /* toggle best-effort */ }
                        }}
                      >
                        <span className="toggle-knob" />
                      </button>
                    </div>
                  ))}
                  {skills.length === 0 && <div className="text-secondary" style={{ textAlign: 'center', padding: 20 }}>暂无已安装的 Skill</div>}
                </div>
                
                <div style={{ marginTop: 24, textAlign: 'right' }}>
                  <button className="btn btn-primary" onClick={() => setShowSkillModal(false)}>完成</button>
                </div>
              </div>
            </div>
          )}
        </div>
      )}

      {phase === 'running' && (
        <div className="review-running">
          <div className="card sandbox-card">
            <div className="sandbox-header">
              <Box size={20} color="var(--accent-cyan)" />
              <h3>{reviewMode === 'ai' ? 'AI 审查执行中' : '快速扫描中'}</h3>
              <div className="loading-spinner" />
            </div>
            <div className="sandbox-logs">
              {runningLog.map((log, i) => (
                <div key={`${log.time}-${i}`} className="sandbox-log-line animate-in" style={{ animationDelay: `${i * 0.05}s` }}>
                  <span className="sandbox-log-time">{log.time}</span>
                  <span>{log.msg}</span>
                </div>
              ))}
            </div>
            <div className="sandbox-status-bar">
              <div className="sandbox-progress">
                <div className="sandbox-progress-fill" style={{ width: `${progressPercent}%` }} />
              </div>
              <span>
                {progressPercent}%
                {totalBatches > 0 && ` (${completedBatches}/${totalBatches} 批)`}
              </span>
            </div>
            <div style={{ display: 'flex', justifyContent: 'flex-start', marginTop: 16 }}>
              <button className="btn btn-secondary" onClick={() => setPhase('configure')}>
                 中断并返回
              </button>
            </div>
          </div>
        </div>
      )}

      {phase === 'results' && (
        <div className="review-results">
          <div className="results-summary">
            <div className="card result-stat" onClick={() => setSeverityFilter('error')} style={{ cursor: 'pointer' }}>
              <XCircle size={24} color="var(--accent-red)" />
              <div className="result-stat-value">{errorCount}</div>
              <div className="result-stat-label">错误</div>
            </div>
            <div className="card result-stat" onClick={() => setSeverityFilter('warning')} style={{ cursor: 'pointer' }}>
              <AlertTriangle size={24} color="var(--accent-orange)" />
              <div className="result-stat-value">{warningCount}</div>
              <div className="result-stat-label">警告</div>
            </div>
            <div className="card result-stat" onClick={() => setSeverityFilter('info')} style={{ cursor: 'pointer' }}>
              <CheckCircle2 size={24} color="var(--accent-blue)" />
              <div className="result-stat-value">{infoCount}</div>
              <div className="result-stat-label">建议</div>
            </div>
            <div className="card result-stat" onClick={() => setSeverityFilter('all')} style={{ cursor: 'pointer' }}>
              <FileCode size={24} color="var(--accent-green)" />
              <div className="result-stat-value">{scannedFiles}</div>
              <div className="result-stat-label">已扫描文件</div>
            </div>
          </div>

          <div className="results-grid">
            <div className="card results-issues">
              {errorMessage ? (
                <div className="empty-state" style={{ padding: '32px 16px', color: 'var(--accent-red)' }}>
                  <XCircle size={32} color="var(--accent-red)" style={{ marginBottom: 16 }} />
                  <h3>审查执行失败</h3>
                  <p style={{ marginTop: 8 }}>{errorMessage}</p>
                </div>
              ) : (
                <>
                  <div className="results-issues-header">
                    <h3>
                      <Filter size={14} style={{ marginRight: 4 }} />
                      问题列表
                      {severityFilter !== 'all' && (
                        <span className={`badge badge-${severityFilter === 'error' ? 'red' : severityFilter === 'warning' ? 'orange' : 'blue'}`} style={{ marginLeft: 8, fontSize: 11 }}>
                          {severityFilter === 'error' ? '仅错误' : severityFilter === 'warning' ? '仅警告' : '仅建议'}
                          <button type="button" style={{ background: 'none', border: 'none', color: 'inherit', cursor: 'pointer', marginLeft: 4, padding: 0 }} onClick={() => setSeverityFilter('all')}>×</button>
                        </span>
                      )}
                    </h3>
                    <div style={{ display: 'flex', gap: 8 }}>
                      <button className="btn btn-sm btn-secondary" onClick={() => void handleCopyReport()}><ClipboardCopy size={14} /> 复制报告</button>
                      <button className="btn btn-sm btn-secondary" onClick={handleExport}><Download size={14} /> 导出</button>
                    </div>
                  </div>
                  {filteredIssues.map((issue, i) => (
                    <div
                      key={`${issue.file}-${issue.line}-${i}`}
                      className={`issue-row ${selectedIssue === i ? 'selected' : ''}`}
                      onClick={() => setSelectedIssue(i)}
                    >
                      <div className="issue-icon">
                        {issue.severity === 'error' && <XCircle size={16} color="var(--accent-red)" />}
                        {issue.severity === 'warning' && <AlertTriangle size={16} color="var(--accent-orange)" />}
                        {issue.severity !== 'error' && issue.severity !== 'warning' && <CheckCircle2 size={16} color="var(--accent-blue)" />}
                      </div>
                      <div className="issue-main">
                        <span className="issue-msg">{issue.message}</span>
                        <span className="issue-loc">{issue.file}:{issue.line}</span>
                      </div>
                      <ChevronRight size={14} color="var(--text-tertiary)" />
                    </div>
                  ))}
                  {filteredIssues.length === 0 && (
                    <div className="empty-state" style={{ padding: '32px 16px' }}>
                      <CheckCircle2 size={32} color="var(--accent-green)" />
                      <h3>{severityFilter === 'all' ? '未发现问题' : '该分类下无问题'}</h3>
                    </div>
                  )}
                </>
              )}
            </div>

            <div className="card results-detail">
              {currentIssue ? (
                <>
                  <h3>修复建议</h3>
                  <div className={`detail-severity badge badge-${currentIssue.severity === 'error' ? 'red' : currentIssue.severity === 'warning' ? 'orange' : 'blue'}`}>
                    {currentIssue.rule}
                  </div>
                  <p className="detail-file">
                    <FileCode size={14} />
                    {currentIssue.file}:{currentIssue.line}
                  </p>
                  <p className="detail-msg">{currentIssue.message}</p>
                  <div className="detail-suggestion">
                    <h4>💡 建议</h4>
                    <p>{currentIssue.suggestion}</p>
                  </div>
                </>
              ) : (
                <div className="empty-state">
                  <SearchCode size={32} />
                  <h3>选择问题查看详情</h3>
                  <p>点击左侧问题查看修复建议</p>
                </div>
              )}
            </div>
          </div>

          <div className="results-actions">
            <button className="btn btn-secondary" onClick={() => setPhase('select')}>
              <RotateCcw size={16} /> 重新审查
            </button>
            <button className="btn btn-primary" onClick={handleAutofix} disabled={errorCount + warningCount === 0}>
              <Play size={16} /> 一键修复全部
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
