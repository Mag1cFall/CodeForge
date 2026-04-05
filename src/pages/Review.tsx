import { SearchCode, FolderOpen, AlertTriangle, CheckCircle2, XCircle, ChevronRight } from 'lucide-react';
import './PageCommon.css';

const mockIssues = [
  { severity: 'error', file: 'src/main.rs', line: 42, message: 'unwrap() on Result without error handling', rule: 'error-handling' },
  { severity: 'warning', file: 'src/lib.rs', line: 87, message: 'Function complexity too high (CC=15)', rule: 'complexity' },
  { severity: 'warning', file: 'src/utils.rs', line: 12, message: 'Unused import: std::collections::HashMap', rule: 'unused-import' },
  { severity: 'info', file: 'src/config.rs', line: 5, message: 'Consider using &str instead of String for read-only parameters', rule: 'best-practice' },
  { severity: 'error', file: 'src/handler.rs', line: 103, message: 'Potential null dereference', rule: 'null-safety' },
];

export default function Review() {
  return (
    <div className="animate-in">
      <div className="page-header">
        <h1>🔍 代码审查</h1>
        <p>AI Agent 驱动的代码质量分析与最佳实践挖掘</p>
      </div>

      <div className="page-toolbar">
        <button className="btn btn-primary">
          <FolderOpen size={16} /> 打开项目
        </button>
        <button className="btn btn-secondary">
          <SearchCode size={16} /> 开始审查
        </button>
      </div>

      <div className="card" style={{ marginTop: 20 }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 20 }}>
          <h3 style={{ fontSize: 15, fontWeight: 600 }}>审查结果</h3>
          <div style={{ display: 'flex', gap: 12 }}>
            <span className="badge badge-red">2 错误</span>
            <span className="badge badge-orange">2 警告</span>
            <span className="badge badge-blue">1 建议</span>
          </div>
        </div>
        <div className="issue-list">
          {mockIssues.map((issue, i) => (
            <div key={i} className="issue-item" style={{ animationDelay: `${i * 0.05}s` }}>
              <div className="issue-icon">
                {issue.severity === 'error' && <XCircle size={16} color="var(--accent-red)" />}
                {issue.severity === 'warning' && <AlertTriangle size={16} color="var(--accent-orange)" />}
                {issue.severity === 'info' && <CheckCircle2 size={16} color="var(--accent-blue)" />}
              </div>
              <div className="issue-info">
                <span className="issue-message">{issue.message}</span>
                <span className="issue-location">{issue.file}:{issue.line} · {issue.rule}</span>
              </div>
              <ChevronRight size={16} className="issue-arrow" />
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
