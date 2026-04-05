import { useEffect, useState } from 'react';
import { Plus, CheckCircle2, Key, Globe, Trash2, Star } from 'lucide-react';
import {
  ProviderConfigInput,
  ProviderSummary,
  providerCreate,
  providerDelete,
  providerList,
} from '../lib/backend';
import './PageCommon.css';
import './Providers.css';

const presets: Record<ProviderConfigInput['providerType'], Pick<ProviderConfigInput, 'endpoint' | 'model' | 'models'>> = {
  openAiCompatible: {
    endpoint: 'https://api.openai.com/v1/chat/completions',
    model: 'gpt-5.4-mini',
    models: ['gpt-5.4-mini', 'gpt-5.4'],
  },
  anthropic: {
    endpoint: 'https://api.anthropic.com/v1',
    model: 'claude-sonnet-4-5',
    models: ['claude-sonnet-4-5', 'claude-opus-4-1'],
  },
};

const initialForm: ProviderConfigInput = {
  name: 'OpenAI Compatible',
  providerType: 'openAiCompatible',
  endpoint: presets.openAiCompatible.endpoint,
  apiKey: '',
  model: presets.openAiCompatible.model,
  models: presets.openAiCompatible.models,
  enabled: true,
  isDefault: true,
  headers: {},
};

export default function Providers() {
  const [providers, setProviders] = useState<ProviderSummary[]>([]);
  const [showForm, setShowForm] = useState(false);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState('');
  const [form, setForm] = useState(initialForm);
  const [modelsText, setModelsText] = useState(initialForm.models.join(', '));

  const loadProviders = async () => {
    setLoading(true);
    setError('');
    try {
      setProviders(await providerList());
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : 'Provider 列表加载失败');
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void loadProviders();
  }, []);

  const handleTypeChange = (providerType: ProviderConfigInput['providerType']) => {
    const preset = presets[providerType];
    setForm((current) => ({
      ...current,
      providerType,
      endpoint: preset.endpoint,
      model: preset.model,
      models: preset.models,
    }));
    setModelsText(preset.models.join(', '));
  };

  const handleSubmit = async () => {
    setSaving(true);
    setError('');
    try {
      await providerCreate({
        ...form,
        models: modelsText.split(',').map((item) => item.trim()).filter(Boolean),
      });
      setForm(initialForm);
      setModelsText(initialForm.models.join(', '));
      setShowForm(false);
      await loadProviders();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : 'Provider 创建失败');
    } finally {
      setSaving(false);
    }
  };

  const handleDelete = async (id: string) => {
    setError('');
    try {
      await providerDelete(id);
      await loadProviders();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : 'Provider 删除失败');
    }
  };

  return (
    <div className="animate-in">
      <div className="page-header">
        <h1>🏗️ 模型配置</h1>
        <p>配置真实的 LLM Provider 连接，当前由 Tauri 后端持久化到 SQLite。</p>
      </div>

      <div className="page-toolbar">
        <button className="btn btn-primary" onClick={() => setShowForm((current) => !current)}>
          <Plus size={16} /> {showForm ? '收起表单' : '添加 Provider'}
        </button>
      </div>

      {showForm && (
        <div className="card provider-form-card">
          <div className="provider-form-grid">
            <div>
              <label>名称</label>
              <input value={form.name} onChange={(event) => setForm((current) => ({ ...current, name: event.target.value }))} />
            </div>
            <div>
              <label>类型</label>
              <select value={form.providerType} onChange={(event) => handleTypeChange(event.target.value as ProviderConfigInput['providerType'])}>
                <option value="openAiCompatible">OpenAI Compatible</option>
                <option value="anthropic">Anthropic</option>
              </select>
            </div>
            <div className="provider-form-span-2">
              <label>端点</label>
              <input value={form.endpoint} onChange={(event) => setForm((current) => ({ ...current, endpoint: event.target.value }))} />
            </div>
            <div>
              <label>默认模型</label>
              <input value={form.model} onChange={(event) => setForm((current) => ({ ...current, model: event.target.value }))} />
            </div>
            <div>
              <label>模型列表</label>
              <input value={modelsText} onChange={(event) => setModelsText(event.target.value)} placeholder="逗号分隔" />
            </div>
            <div className="provider-form-span-2">
              <label>API Key</label>
              <input type="password" value={form.apiKey ?? ''} onChange={(event) => setForm((current) => ({ ...current, apiKey: event.target.value }))} />
            </div>
          </div>
          <div className="provider-form-actions">
            <label className="provider-checkbox">
              <input type="checkbox" checked={form.enabled} onChange={(event) => setForm((current) => ({ ...current, enabled: event.target.checked }))} />
              启用
            </label>
            <label className="provider-checkbox">
              <input type="checkbox" checked={form.isDefault} onChange={(event) => setForm((current) => ({ ...current, isDefault: event.target.checked }))} />
              设为默认
            </label>
            <button className="btn btn-primary" onClick={handleSubmit} disabled={saving}>
              <Plus size={16} /> {saving ? '保存中...' : '保存 Provider'}
            </button>
          </div>
        </div>
      )}

      {error && <div className="provider-error">{error}</div>}

      {loading ? (
        <div className="card empty-state"><p>正在加载 Provider...</p></div>
      ) : providers.length === 0 ? (
        <div className="card empty-state">
          <h3>还没有 Provider</h3>
          <p>先添加一个默认 Provider，聊天和审查功能才能调用真实模型。</p>
        </div>
      ) : (
        <div className="card-grid">
          {providers.map((provider) => (
            <div key={provider.id} className="card card-glow provider-card">
              <div className="provider-card-header">
                <h4>{provider.name}</h4>
                <div className="provider-card-badges">
                  {provider.isDefault && <span className="badge badge-purple"><Star size={12} /> 默认</span>}
                  {provider.enabled ? (
                    <span className="badge badge-green"><CheckCircle2 size={12} /> 可用</span>
                  ) : (
                    <span className="badge badge-red">未激活</span>
                  )}
                </div>
              </div>
              <div className="provider-endpoint">
                <Globe size={12} /> {provider.endpoint}
              </div>
              <div className="provider-models">
                {provider.models.length > 0 ? provider.models : [provider.model]}
              </div>
              <div className="provider-meta">
                <span style={{ color: provider.keySet ? 'var(--accent-green-light)' : 'var(--text-tertiary)' }}>
                  <Key size={12} /> {provider.keySet ? 'API Key 已配置' : '未设置 API Key'}
                </span>
                <button className="btn btn-sm btn-danger" onClick={() => void handleDelete(provider.id)}>
                  <Trash2 size={14} /> 删除
                </button>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
