import { useEffect, useState } from 'react';
import { Plus, CheckCircle2, Key, Globe, Server } from 'lucide-react';
import { useAppPreferences } from '../lib/app-preferences';
import { providerList, ProviderSummary } from '../lib/backend';
import './PageCommon.css';

export default function Providers() {
  const { t } = useAppPreferences();
  const [providers, setProviders] = useState<ProviderSummary[]>([]);

  useEffect(() => {
    const loadProviders = async () => {
      try {
        const data = await providerList();
        setProviders(data ?? []);
      } catch {
        setProviders([]);
      }
    };

    void loadProviders();
  }, []);

  return (
    <div className="animate-in">
      <div className="page-header">
        <h1><Server size={28} style={{ verticalAlign: 'middle', marginRight: 8 }} /> {t('route.providers')}</h1>
        <p>{t('page.providers.desc')}</p>
      </div>

      <div className="page-toolbar">
        <button type="button" className="btn btn-primary"><Plus size={16} /> 添加 Provider</button>
      </div>

      <div className="card-grid">
        {providers.map((p) => (
          <div key={p.id} className="card card-glow">
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 16 }}>
              <h4 style={{ fontSize: 16, fontWeight: 700 }}>{p.name}</h4>
              {p.enabled
                ? <span className="badge badge-green"><CheckCircle2 size={12} /> 可用</span>
                : <span className="badge badge-red">未激活</span>
              }
            </div>
            <div style={{ fontSize: 12, color: 'var(--text-tertiary)', fontFamily: 'var(--font-mono)', marginBottom: 12 }}>
              <Globe size={12} style={{ verticalAlign: 'middle', marginRight: 4 }} />
              {p.endpoint}
            </div>
            <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap', marginBottom: 12 }}>
              {(p.models.length > 0 ? p.models : [p.model]).map((m) => (
                <span key={m} className="badge badge-purple">{m}</span>
              ))}
            </div>
            <div style={{ fontSize: 12, color: p.keySet ? 'var(--accent-green-light)' : 'var(--text-tertiary)' }}>
              <Key size={12} style={{ verticalAlign: 'middle', marginRight: 4 }} />
              {p.keySet ? 'API Key 已配置' : '未设置 API Key'}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
