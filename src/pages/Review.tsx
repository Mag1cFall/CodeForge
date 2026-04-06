import { useState } from 'react';
import {
  SearchCode, FolderOpen, GitBranch, Play,
  AlertTriangle, CheckCircle2, XCircle, ChevronRight,
  Shield, Box, FileCode, RotateCcw,
  Download, ClipboardCopy, Settings2
} from 'lucide-react';
import './Review.css';

type Phase = 'select' | 'configure' | 'running' | 'results';

const mockIssues = [
  { severity: 'error' as const, file: 'src/main.rs', line: 42, message: 'unwrap() on Result without error handling', rule: 'error-handling', suggestion: '使用 ? 运算符或 match 处理错误' },
  { severity: 'error' as const, file: 'src/handler.rs', line: 103, message: 'Potential null pointer dereference', rule: 'null-safety', suggestion: '添加 Option::is_some() 检查' },
  { severity: 'warning' as const, file: 'src/lib.rs', line: 87, message: 'Function cyclomatic complexity: 15 (threshold: 10)', rule: 'complexity', suggestion: '拆分为 validate_input() + transform_data() + output_result()' },
  { severity: 'warning' as const, file: 'src/utils.rs', line: 12, message: 'Unused import: std::collections::HashMap', rule: 'unused-import', suggestion: '删除未使用的 import' },
  { severity: 'info' as const, file: 'src/config.rs', line: 5, message: 'Consider &str over String for read-only parameters', rule: 'best-practice', suggestion: 'fn load_config(path: &str) 替代 fn load_config(path: String)' },
  { severity: 'info' as const, file: 'src/types.rs', line: 28, message: 'Derive Debug trait for better debugging', rule: 'best-practice', suggestion: '添加 #[derive(Debug)] 到所有 struct' },
];

const sandboxLogs = [
  { time: '00:00.0', msg: '🔒 沙箱环境初始化...' },
  { time: '00:00.2', msg: '📁 挂载项目目录 (只读)' },
  { time: '00:00.5', msg: '[Agent] Reviewer Agent 启动' },
  { time: '00:01.2', msg: '🔧 Tool: list_directory → 42 files found' },
  { time: '00:02.8', msg: '🔧 Tool: analyze_ast → src/main.rs (CC=15)' },
  { time: '00:04.1', msg: '🔧 Tool: find_code_smells → 6 issues detected' },
  { time: '00:05.5', msg: '🔧 Tool: search_code → pattern: unwrap()' },
  { time: '00:07.0', msg: '📊 Agent 生成审查报告...' },
  { time: '00:08.3', msg: '✅ 审查完成' },
];

export default function Review() {
  const [phase, setPhase] = useState<Phase>('select');
  const [projectPath, setProjectPath] = useState('');
  const [gitUrl, setGitUrl] = useState('');
  const [selectedAgent, setSelectedAgent] = useState('Reviewer');
  const [sandboxEnabled, setSandboxEnabled] = useState(true);
  const [runningLog, setRunningLog] = useState<typeof sandboxLogs>([]);
  const [selectedIssue, setSelectedIssue] = useState<number | null>(null);

  const startReview = () => {
    setPhase('running');
    setRunningLog([]);
    sandboxLogs.forEach((log, i) => {
      setTimeout(() => {
        setRunningLog(prev => [...prev, log]);
        if (i === sandboxLogs.length - 1) {
          setTimeout(() => setPhase('results'), 800);
        }
      }, (i + 1) * 600);
    });
  };

  return (
    <div className="review-page animate-in">
      <div className="page-header">
        <h1><SearchCode size={28} style={{ verticalAlign: 'middle', marginRight: 8 }} /> 代码审查</h1>
        <p>Agent 驱动的代码质量分析 · 沙箱隔离执行 · 完整审查报告输出</p>
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
              }
            } catch {
              console.warn('plugin-dialog failed, please enter manually');
            }
          }}>
            <FolderOpen size={32} color="var(--accent-blue-light)" />
            <h3>本地文件夹</h3>
            <p>选择本地项目目录进行审查</p>
            <div style={{ display: 'flex', gap: 8 }} onClick={e => e.stopPropagation()}>
              <input
                placeholder="项目路径..."
                value={projectPath}
                onChange={e => setProjectPath(e.target.value)}
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
                onChange={e => setGitUrl(e.target.value)}
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
                <div key={i} className="sandbox-log-line animate-in" style={{ animationDelay: `${i * 0.05}s` }}>
                  <span className="sandbox-log-time">{log.time}</span>
                  <span>{log.msg}</span>
                </div>
              ))}
            </div>
            <div className="sandbox-status-bar">
              <div className="sandbox-progress">
                <div className="sandbox-progress-fill" style={{ width: `${(runningLog.length / sandboxLogs.length) * 100}%` }} />
              </div>
              <span>{Math.round((runningLog.length / sandboxLogs.length) * 100)}%</span>
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
              <div className="result-stat-value">2</div>
              <div className="result-stat-label">错误</div>
            </div>
            <div className="card result-stat">
              <AlertTriangle size={24} color="var(--accent-orange)" />
              <div className="result-stat-value">2</div>
              <div className="result-stat-label">警告</div>
            </div>
            <div className="card result-stat">
              <CheckCircle2 size={24} color="var(--accent-blue)" />
              <div className="result-stat-value">2</div>
              <div className="result-stat-label">建议</div>
            </div>
            <div className="card result-stat">
              <FileCode size={24} color="var(--accent-green)" />
              <div className="result-stat-value">42</div>
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
              {mockIssues.map((issue, i) => (
                <div
                  key={i}
                  className={`issue-row ${selectedIssue === i ? 'selected' : ''}`}
                  onClick={() => setSelectedIssue(i)}
                >
                  <div className="issue-icon">
                    {issue.severity === 'error' && <XCircle size={16} color="var(--accent-red)" />}
                    {issue.severity === 'warning' && <AlertTriangle size={16} color="var(--accent-orange)" />}
                    {issue.severity === 'info' && <CheckCircle2 size={16} color="var(--accent-blue)" />}
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
              {selectedIssue !== null ? (
                <>
                  <h3>修复建议</h3>
                  <div className={`detail-severity badge badge-${mockIssues[selectedIssue].severity === 'error' ? 'red' : mockIssues[selectedIssue].severity === 'warning' ? 'orange' : 'blue'}`}>
                    {mockIssues[selectedIssue].rule}
                  </div>
                  <p className="detail-file">
                    <FileCode size={14} />
                    {mockIssues[selectedIssue].file}:{mockIssues[selectedIssue].line}
                  </p>
                  <p className="detail-msg">{mockIssues[selectedIssue].message}</p>
                  <div className="detail-suggestion">
                    <h4>💡 建议</h4>
                    <p>{mockIssues[selectedIssue].suggestion}</p>
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
