import { useState, useEffect, useCallback, Fragment } from 'react';
import { ScrollText, Filter, ChevronRight, CheckCircle2, XCircle, ChevronDown } from 'lucide-react';
import { useAppPreferences } from '../lib/app-preferences';
import { logList as fetchLogList, TraceLog } from '../lib/backend';
import './PageCommon.css';

interface LogRow {
  id: string;
  agent: string;
  action: string;
  status: 'success' | 'error';
  tokens: number;
  time: string;
  duration: string;
  payload: Record<string, unknown>;
}

const isRecord = (value: unknown): value is Record<string, unknown> => {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
};

const readString = (value: Record<string, unknown>, key: string): string => {
  const target = value[key];
  return typeof target === 'string' ? target : '';
};

const readNumber = (value: Record<string, unknown>, key: string): number | null => {
  const target = value[key];
  return typeof target === 'number' ? target : null;
};

const toTimeText = (isoText: string): string => {
  const parsed = new Date(isoText);
  if (Number.isNaN(parsed.getTime())) {
    return isoText;
  }
  return parsed.toLocaleString('zh-CN');
};

const toLogRow = (log: TraceLog): LogRow => {
  const payload = isRecord(log.payload) ? log.payload : {};
  const actionField = readString(payload, 'action') || readString(payload, 'name') || readString(payload, 'content') || readString(payload, 'query');
  const statusField = readString(payload, 'status').toLowerCase();

  return {
    id: `tr-${String(log.id).padStart(3, '0')}`,
    agent: readString(payload, 'agent') || log.kind,
    action: actionField ? `${log.kind} → ${actionField}` : log.kind,
    status: statusField === 'error' || log.kind.includes('error') ? 'error' : 'success',
    tokens: readNumber(payload, 'tokens') ?? readNumber(payload, 'tokenCount') ?? 0,
    time: toTimeText(log.createdAt),
    duration: readString(payload, 'duration') || '-',
    payload,
  };
};

export default function Logs() {
  const { t } = useAppPreferences();
  const [limit, setLimit] = useState(20);
  const [logList, setLogList] = useState<LogRow[]>([]);
  const [showFilter, setShowFilter] = useState(false);
  const [expandedRow, setExpandedRow] = useState<string | null>(null);

  const loadLogs = useCallback(async (targetLimit: number) => {
    try {
      const data = await fetchLogList(targetLimit);
      setLogList((data ?? []).map(toLogRow));
    } catch {
      setLogList([]);
    }
  }, []);

  useEffect(() => {
    void loadLogs(limit);
  }, [limit, loadLogs]);

  const handleLoadMore = () => {
    setLimit((prev) => prev + 20);
  };

  const totalTokens = logList.reduce((sum, log) => sum + log.tokens, 0);

  return (
    <div className="animate-in">
      <div className="page-header">
        <h1><ScrollText size={28} style={{ verticalAlign: 'middle', marginRight: 8 }} /> {t('route.logs')}</h1>
        <p>{t('page.logs.desc')}</p>
      </div>

      <div className="page-toolbar">
        <button className={`btn ${showFilter ? 'btn-primary' : 'btn-secondary'}`} onClick={() => setShowFilter(!showFilter)}>
          <Filter size={16} /> 筛选
        </button>
        <div style={{ marginLeft: 'auto', fontSize: 13, color: 'var(--text-secondary)' }}>
          总计消耗: <strong style={{ color: 'var(--accent-orange-light)' }}>{totalTokens.toLocaleString('zh-CN')}</strong> tokens
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
                          {JSON.stringify(log.payload, null, 2)}
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
