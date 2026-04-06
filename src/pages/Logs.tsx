import { useState, Fragment } from 'react';
import { ScrollText, Filter, ChevronRight, CheckCircle2, XCircle, ChevronDown } from 'lucide-react';
import './PageCommon.css';

const logs = [
  { id: 'tr-001', agent: 'Orchestrator', action: '代码审查任务分发', status: 'success', tokens: 1240, time: '2026-04-06 00:05:12', duration: '3.2s' },
  { id: 'tr-002', agent: 'Reviewer', action: 'analyze_ast → src/main.rs', status: 'success', tokens: 890, time: '2026-04-06 00:05:15', duration: '1.8s' },
  { id: 'tr-003', agent: 'Reviewer', action: 'find_code_smells → 5 issues', status: 'success', tokens: 1560, time: '2026-04-06 00:05:17', duration: '2.4s' },
  { id: 'tr-004', agent: 'Executor', action: 'run_shell → cargo test', status: 'error', tokens: 450, time: '2026-04-06 00:05:20', duration: '15.1s' },
  { id: 'tr-005', agent: 'Refactorer', action: 'suggest_refactor → extract_function', status: 'success', tokens: 2100, time: '2026-04-06 00:05:35', duration: '4.7s' },
];

export default function Logs() {
  const [logList, setLogList] = useState(logs);
  const [showFilter, setShowFilter] = useState(false);
  const [expandedRow, setExpandedRow] = useState<string | null>(null);

  const handleLoadMore = () => {
    setLogList([...logList, {
      id: `tr-00${logList.length + 1}`,
      agent: 'System',
      action: 'Older logs loaded',
      status: 'success',
      tokens: 0,
      time: '2026-04-06 00:00:00',
      duration: '0s'
    }]);
  };

  return (
    <div className="animate-in">
      <div className="page-header">
        <h1><ScrollText size={28} style={{ verticalAlign: 'middle', marginRight: 8 }} /> 执行日志</h1>
        <p>Agent 执行 trace、工具调用日志、Token 消耗明细</p>
      </div>

      <div className="page-toolbar">
        <button className={`btn ${showFilter ? 'btn-primary' : 'btn-secondary'}`} onClick={() => setShowFilter(!showFilter)}>
          <Filter size={16} /> 筛选
        </button>
        <div style={{ marginLeft: 'auto', fontSize: 13, color: 'var(--text-secondary)' }}>
          总计消耗: <strong style={{ color: 'var(--accent-orange-light)' }}>6,240</strong> tokens
        </div>
      </div>

      {showFilter && (
        <div className="card" style={{ marginTop: 20 }}>
          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: 16 }}>
            <div>
              <label>Agent</label>
              <select style={{ width: '100%', padding: '0.4rem', background: 'var(--bg-main)', border: '1px solid var(--border-color)', color: 'var(--text-primary)', borderRadius: '4px' }}>
                <option value="all">所有</option>
                <option value="Orchestrator">Orchestrator</option>
                <option value="Reviewer">Reviewer</option>
              </select>
            </div>
            <div>
              <label>状态</label>
              <select style={{ width: '100%', padding: '0.4rem', background: 'var(--bg-main)', border: '1px solid var(--border-color)', color: 'var(--text-primary)', borderRadius: '4px' }}>
                <option value="all">所有</option>
                <option value="success">成功</option>
                <option value="error">失败</option>
              </select>
            </div>
            <div>
              <label>时间范围</label>
              <select style={{ width: '100%', padding: '0.4rem', background: 'var(--bg-main)', border: '1px solid var(--border-color)', color: 'var(--text-primary)', borderRadius: '4px' }}>
                <option value="today">今天</option>
                <option value="week">本周</option>
              </select>
            </div>
          </div>
        </div>
      )}

      <div className="table-card card" style={{ marginTop: 20 }}>
        <table className="data-table">
          <thead>
            <tr><th>ID</th><th>Agent</th><th>操作</th><th>状态</th><th>Tokens</th><th>耗时</th><th>时间</th></tr>
          </thead>
          <tbody>
            {logList.map((log) => (
              <Fragment key={log.id}>
                <tr onClick={() => setExpandedRow(expandedRow === log.id ? null : log.id)} style={{ cursor: 'pointer' }}>
                  <td style={{ fontFamily: 'var(--font-mono)', fontSize: 12 }}>
                    {expandedRow === log.id ? <ChevronDown size={14} style={{ verticalAlign: 'middle' }} /> : <ChevronRight size={14} style={{ verticalAlign: 'middle' }} />}
                    {' '} {log.id}
                  </td>
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
                {expandedRow === log.id && (
                  <tr>
                    <td colSpan={7} style={{ background: 'var(--bg-main)' }}>
                      <div style={{ padding: 16 }}>
                        <div style={{ fontSize: 14, fontWeight: 'bold', marginBottom: 8 }}>详细执行参数和结果</div>
                        <pre style={{ fontSize: 12, color: 'var(--accent-green-light)', background: 'var(--bg-card)', padding: 12, borderRadius: 6, border: '1px solid var(--border-color)', overflowX: 'auto' }}>
                          {'"args": {\n  "pattern": "unwrap()",\n  "path": "/src/"\n}\n"result": {\n  "matches": 5,\n  "files": 2\n}'}
                        </pre>
                      </div>
                    </td>
                  </tr>
                )}
              </Fragment>
            ))}
          </tbody>
        </table>
      </div>
      
      <div style={{ display: 'flex', justifyContent: 'center', marginTop: 20 }}>
        <button className="btn btn-secondary" onClick={handleLoadMore}>加载更多日志</button>
      </div>
    </div>
  );
}
