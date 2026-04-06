import { useState, useEffect } from 'react';
import {
  SearchCode, FolderOpen, GitBranch, Play,
  AlertTriangle, CheckCircle2, XCircle, ChevronRight,
  Shield, Box, FileCode, RotateCcw,
  Download, ClipboardCopy, Settings2
} from 'lucide-react';
import {
  projectOpen,
  projectClone,
  projectReview,
  listenReviewProgress,
  listenReviewResult,
  ReviewIssue,
} from '../lib/backend';
import { useAppPreferences } from '../lib/app-preferences';
import './Review.css';

type Phase = 'select' | 'configure' | 'running' | 'results';

interface RunningLogItem {
  time: string;
  msg: string;
}

const nowTime = (): string => {
  return new Date().toLocaleTimeString('zh-CN', { hour12: false });
};

export default function Review() {
  const { t } = useAppPreferences();
  const [phase, setPhase] = useState<Phase>('select');
  const [projectPath, setProjectPath] = useState('');
  const [gitUrl, setGitUrl] = useState('');
  const [selectedAgent, setSelectedAgent] = useState('Reviewer');
  const [sandboxEnabled, setSandboxEnabled] = useState(true);
  const [runningLog, setRunningLog] = useState<RunningLogItem[]>([]);
  const [issues, setIssues] = useState<ReviewIssue[]>([]);
  const [selectedIssue, setSelectedIssue] = useState<number | null>(null);
  const [scannedFiles, setScannedFiles] = useState(0);

  useEffect(() => {
    let unlistenProgress: (() => void) | null = null;
    let unlistenResult: (() => void) | null = null;

    const subscribe = async () => {
      try {
        unlistenProgress = await listenReviewProgress((payload) => {
          setRunningLog((prev) => [...prev, { time: nowTime(), msg: payload.log }]);
        });
      } catch {
        unlistenProgress = null;
      }

      try {
        unlistenResult = await listenReviewResult((payload) => {
          setIssues(payload ?? []);
          setPhase('results');
        });
      } catch {
        unlistenResult = null;
      }
    };

    void subscribe();
    return () => {
      if (unlistenProgress) {
        unlistenProgress();
      }
      if (unlistenResult) {
        unlistenResult();
      }
    };
  }, []);

  const startReview = () => {
    void (async () => {
      setPhase('running');
      setRunningLog([]);
      setIssues([]);
      setSelectedIssue(null);

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

        await projectReview(reviewPath, sandboxEnabled);
        setPhase('results');
      } catch {
        setIssues([]);
        setPhase('results');
      }
    })();
  };

  const errorCount = issues.filter((issue) => issue.severity === 'error').length;
  const warningCount = issues.filter((issue) => issue.severity === 'warning').length;
  const infoCount = issues.filter((issue) => issue.severity !== 'error' && issue.severity !== 'warning').length;
  const progressPercent = Math.min(95, runningLog.length * 20);
  const currentIssue = selectedIssue !== null ? issues[selectedIssue] : null;

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
            } catch {
            }
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
                <label>Agent</label>
                <select value={selectedAgent} onChange={e => setSelectedAgent(e.target.value)}>
                  <option>Reviewer</option>
                  <option>Orchestrator (多 Agent 协作)</option>
                </select>
              </div>
              <div className="config-item">
                <label>审查范围</label>
                <select defaultValue="all">
                  <option value="all">全部文件</option>
                  <option value="changed">仅修改文件 (git diff)</option>
                  <option value="src">仅 src/ 目录</option>
                </select>
              </div>
              <div className="config-item">
                <label>
                  <Shield size={14} /> 沙箱隔离
                </label>
                <div className="toggle-row">
                  <button
                    className={`toggle-btn ${sandboxEnabled ? 'on' : ''}`}
                    onClick={() => setSandboxEnabled(!sandboxEnabled)}
                  >
                    {sandboxEnabled ? '启用' : '禁用'}
                  </button>
                  <span className="config-hint">
                    {sandboxEnabled ? '项目目录只读挂载，所有修改在隔离环境执行' : '⚠️ Agent 可直接修改文件'}
                  </span>
                </div>
              </div>
              <div className="config-item">
                <label>启用的 Skill</label>
                <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
                  {['代码审查', '最佳实践', '安全审计'].map(s => (
                    <span key={s} className="badge badge-blue">{s}</span>
                  ))}
                </div>
              </div>
            </div>
            <div className="config-actions">
              <button className="btn btn-secondary" onClick={() => setPhase('select')}>返回</button>
              <button className="btn btn-primary" onClick={startReview}>
                <Play size={16} /> 开始审查
              </button>
            </div>
          </div>
        </div>
      )}

      {phase === 'running' && (
        <div className="review-running">
          <div className="card sandbox-card">
            <div className="sandbox-header">
              <Box size={20} color="var(--accent-cyan)" />
              <h3>沙箱执行中</h3>
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
              <span>{progressPercent}%</span>
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
            <div className="card result-stat">
              <XCircle size={24} color="var(--accent-red)" />
              <div className="result-stat-value">{errorCount}</div>
              <div className="result-stat-label">错误</div>
            </div>
            <div className="card result-stat">
              <AlertTriangle size={24} color="var(--accent-orange)" />
              <div className="result-stat-value">{warningCount}</div>
              <div className="result-stat-label">警告</div>
            </div>
            <div className="card result-stat">
              <CheckCircle2 size={24} color="var(--accent-blue)" />
              <div className="result-stat-value">{infoCount}</div>
              <div className="result-stat-label">建议</div>
            </div>
            <div className="card result-stat">
              <FileCode size={24} color="var(--accent-green)" />
              <div className="result-stat-value">{scannedFiles}</div>
              <div className="result-stat-label">已扫描文件</div>
            </div>
          </div>

          <div className="results-grid">
            <div className="card results-issues">
              <div className="results-issues-header">
                <h3>问题列表</h3>
                <div style={{ display: 'flex', gap: 8 }}>
                  <button className="btn btn-sm btn-secondary"><ClipboardCopy size={14} /> 复制报告</button>
                  <button className="btn btn-sm btn-secondary"><Download size={14} /> 导出</button>
                </div>
              </div>
              {issues.map((issue, i) => (
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
            <button className="btn btn-primary">
              <Play size={16} /> 一键修复全部
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
