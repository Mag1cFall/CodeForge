import { ShieldAlert, Check, X, Terminal } from 'lucide-react';
import './PermissionDialog.css';

export interface PermissionRequest {
  id: string;
  toolName: string;
  args: Record<string, unknown>;
  riskLevel: 'low' | 'medium' | 'high';
  description: string;
}

interface Props {
  request: PermissionRequest | null;
  onApprove: (id: string) => void;
  onDeny: (id: string) => void;
}

export default function PermissionDialog({ request, onApprove, onDeny }: Props) {
  if (!request) return null;

  const riskColors = {
    low: 'var(--accent-green)',
    medium: 'var(--accent-orange)',
    high: 'var(--accent-red)',
  };

  const riskLabels = { low: '低风险', medium: '中风险', high: '高风险' };

  return (
    <div className="perm-overlay">
      <div className="perm-dialog animate-in">
        <div className="perm-header">
          <ShieldAlert size={24} color={riskColors[request.riskLevel]} />
          <div>
            <h3>Agent 请求权限</h3>
            <span className={`badge badge-${request.riskLevel === 'high' ? 'red' : request.riskLevel === 'medium' ? 'orange' : 'green'}`}>
              {riskLabels[request.riskLevel]}
            </span>
          </div>
        </div>

        <div className="perm-body">
          <p className="perm-desc">{request.description}</p>
          <div className="perm-tool">
            <Terminal size={14} />
            <code>{request.toolName}</code>
          </div>
          <pre className="perm-args">{JSON.stringify(request.args, null, 2)}</pre>
        </div>

        <div className="perm-actions">
          <button type="button" className="btn btn-danger" onClick={() => onDeny(request.id)}>
            <X size={16} /> 拒绝
          </button>
          <button type="button" className="btn btn-primary" onClick={() => onApprove(request.id)}>
            <Check size={16} /> 允许执行
          </button>
        </div>
      </div>
    </div>
  );
}
