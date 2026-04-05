import { ScrollText, Filter, ChevronRight, CheckCircle2, XCircle, Clock } from 'lucide-react';
import './PageCommon.css';

const logs = [
  { id: 'tr-001', agent: 'Orchestrator', action: '代码审查任务分发', status: 'success', tokens: 1240, time: '2026-04-06 00:05:12', duration: '3.2s' },
  { id: 'tr-002', agent: 'Reviewer', action: 'analyze_ast → src/main.rs', status: 'success', tokens: 890, time: '2026-04-06 00:05:15', duration: '1.8s' },
  { id: 'tr-003', agent: 'Reviewer', action: 'find_code_smells → 5 issues', status: 'success', tokens: 1560, time: '2026-04-06 00:05:17', duration: '2.4s' },
  { id: 'tr-004', agent: 'Executor', action: 'run_shell → cargo test', status: 'error', tokens: 450, time: '2026-04-06 00:05:20', duration: '15.1s' },
  { id: 'tr-005', agent: 'Refactorer', action: 'suggest_refactor → extract_function', status: 'success', tokens: 2100, time: '2026-04-06 00:05:35', duration: '4.7s' },
];

export default function Logs() {
  return (
    <div className="animate-in">
      <div className="page-header">
        <h1>📋 执行日志</h1>
        <p>Agent 执行 trace、工具调用日志、Token 消耗明细</p>
      </div>

      <div className="page-toolbar">
        <button className="btn btn-secondary"><Filter size={16} /> 筛选</button>
        <div style={{ marginLeft: 'auto', fontSize: 13, color: 'var(--text-secondary)' }}>
          总计消耗: <strong style={{ color: 'var(--accent-orange-light)' }}>6,240</strong> tokens
        </div>
      </div>

      <div className="table-card card" style={{ marginTop: 20 }}>
        <table className="data-table">
          <thead>
            <tr><th>ID</th><th>Agent</th><th>操作</th><th>状态</th><th>Tokens</th><th>耗时</th><th>时间</th></tr>
          </thead>
          <tbody>
            {logs.map((log) => (
              <tr key={log.id}>
                <td style={{ fontFamily: 'var(--font-mono)', fontSize: 12 }}>{log.id}</td>
                <td><span className="badge badge-blue">{log.agent}</span></td>
                <td style={{ fontSize: 13 }}>{log.action}</td>
                <td>
                  {log.status === 'success'
                    ? <span className="badge badge-green"><CheckCircle2 size={12} /> 成功</span>
                    : <span className="badge badge-red"><XCircle size={12} /> 失败</span>
                  }
                </td>
                <td style={{ fontFamily: 'var(--font-mono)', fontSize: 13 }}>{log.tokens}</td>
                <td style={{ fontSize: 13 }}>{log.duration}</td>
                <td style={{ fontSize: 12, color: 'var(--text-tertiary)' }}>{log.time}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
