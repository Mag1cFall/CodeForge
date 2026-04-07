import { useCallback, useEffect, useMemo, useState } from 'react';
import { Plus, CheckCircle2, Key, Globe, Server, Trash2, Pencil, RefreshCw, X } from 'lucide-react';
import { useAppPreferences } from '../lib/app-preferences';
import ConfirmDialog from '../components/ConfirmDialog';
import {
  providerCreate,
  providerDelete,
  providerFetchModels,
  providerList,
  providerUpdate,
  ProviderSummary,
  ProviderType,
} from '../lib/backend';
import './PageCommon.css';

interface ProviderFormState {
  name: string;
  providerType: ProviderType;
  endpoint: string;
  apiKey: string;
  defaultModel: string;
  modelInput: string;
  models: string[];
}

const createEmptyForm = (): ProviderFormState => ({
  name: '',
  providerType: 'openAiCompatible',
  endpoint: '',
  apiKey: '',
  defaultModel: '',
  modelInput: '',
  models: [],
});

const normalizeModelList = (items: string[]): string[] => {
  const dedupe = new Set<string>();
  const output: string[] = [];
  for (const item of items) {
    const model = item.trim();
    if (!model || dedupe.has(model)) {
      continue;
    }
    dedupe.add(model);
    output.push(model);
  }
  return output;
};

export default function Providers() {
  const { t } = useAppPreferences();
  const [providers, setProviders] = useState<ProviderSummary[]>([]);
  const [showForm, setShowForm] = useState(false);
  const [isDeleting, setIsDeleting] = useState(false);
  const [isFetchingModels, setIsFetchingModels] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<ProviderSummary | null>(null);
  const [form, setForm] = useState<ProviderFormState>(createEmptyForm);

  const loadProviders = useCallback(async () => {
    try {
      const data = await providerList();
      setProviders(data ?? []);
    } catch {
      setProviders([]);
    }
  }, []);

  useEffect(() => {
    void loadProviders();
  }, [loadProviders]);

  const editingProvider = useMemo(
    () => providers.find((item) => item.id === editingId) || null,
    [providers, editingId],
  );

  const resetForm = useCallback(() => {
    setForm(createEmptyForm());
    setEditingId(null);
  }, []);

  const handleToggleForm = useCallback(() => {
    if (showForm) {
      setShowForm(false);
      resetForm();
      return;
    }
    setShowForm(true);
  }, [showForm, resetForm]);

  const handleAddModel = useCallback(() => {
    const model = form.modelInput.trim();
    if (!model) {
      return;
    }

    setForm((prev) => {
      const models = normalizeModelList([...prev.models, model]);
      return {
        ...prev,
        models,
        defaultModel: prev.defaultModel || model,
        modelInput: '',
      };
    });
  }, [form.modelInput]);

  const handleRemoveModel = useCallback((model: string) => {
    setForm((prev) => {
      const models = prev.models.filter((item) => item !== model);
      const defaultModel = prev.defaultModel === model ? (models[0] ?? '') : prev.defaultModel;
      return {
        ...prev,
        models,
        defaultModel,
      };
    });
  }, []);

  const handleFetchModels = useCallback(async () => {
    if (!form.endpoint.trim() || isFetchingModels) {
      return;
    }

    setIsFetchingModels(true);
    try {
      const fetched = await providerFetchModels(
        form.providerType,
        form.endpoint.trim(),
        form.apiKey.trim() || null,
      );

      setForm((prev) => {
        const merged = normalizeModelList([...prev.models, ...(fetched ?? [])]);
        return {
          ...prev,
          models: merged,
          defaultModel: merged.includes(prev.defaultModel) ? prev.defaultModel : (merged[0] ?? ''),
        };
      });
    } catch {
    } finally {
      setIsFetchingModels(false);
    }
  }, [form.endpoint, form.providerType, form.apiKey, isFetchingModels]);

  const handleEditProvider = useCallback((provider: ProviderSummary) => {
    const models = normalizeModelList(provider.models.length > 0 ? provider.models : [provider.model]);
    setShowForm(true);
    setEditingId(provider.id);
    setForm({
      name: provider.name,
      providerType: provider.providerType,
      endpoint: provider.endpoint,
      apiKey: provider.apiKey ?? '',
      defaultModel: provider.model,
      modelInput: '',
      models,
    });
  }, []);

  const handleSaveProvider = useCallback(() => {
    if (!form.name.trim() || !form.endpoint.trim()) {
      return;
    }

    void (async () => {
      try {
        const draftModels = form.modelInput.trim()
          ? normalizeModelList([...form.models, form.modelInput.trim()])
          : normalizeModelList(form.models);
        if (draftModels.length === 0) {
          return;
        }

        const defaultModel = draftModels.includes(form.defaultModel.trim())
          ? form.defaultModel.trim()
          : draftModels[0];

        const payload = {
          name: form.name.trim(),
          providerType: form.providerType,
          endpoint: form.endpoint.trim(),
          apiKey: form.apiKey.trim() || null,
          model: defaultModel,
          models: draftModels,
          enabled: true,
          isDefault: editingProvider?.isDefault ?? providers.length === 0,
          headers: {},
        };

        if (editingId) {
          await providerUpdate(editingId, payload);
        } else {
          await providerCreate(payload);
        }

        setShowForm(false);
        resetForm();
        await loadProviders();
      } catch {
        setProviders([]);
      }
    })();
  }, [editingId, editingProvider?.isDefault, form, providers.length, loadProviders, resetForm]);

  const handleConfirmDelete = useCallback(async () => {
    if (!deleteTarget || isDeleting) {
      return;
    }
    setIsDeleting(true);
    try {
      await providerDelete(deleteTarget.id);
      setDeleteTarget(null);
      await loadProviders();
    } catch {
      setProviders([]);
    } finally {
      setIsDeleting(false);
    }
  }, [deleteTarget, isDeleting, loadProviders]);

  return (
    <div className="animate-in">
      <div className="page-header">
        <h1><Server size={28} style={{ verticalAlign: 'middle', marginRight: 8 }} /> {t('route.providers')}</h1>
        <p>{t('page.providers.desc')}</p>
      </div>

      <div className="page-toolbar">
        <button type="button" className="btn btn-primary" onClick={handleToggleForm}>
          <Plus size={16} /> {showForm ? '取消' : '添加 Provider'}
        </button>
      </div>

      {showForm && (
        <div className="card" style={{ marginBottom: 20 }}>
          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 16 }}>
            <div>
              <label htmlFor="provider-name">名称</label>
              <input
                id="provider-name"
                value={form.name}
                onChange={(e) => setForm((prev) => ({ ...prev, name: e.target.value }))}
                style={{ width: '100%' }}
              />
            </div>
            <div>
              <label htmlFor="provider-type">类型</label>
              <select
                id="provider-type"
                value={form.providerType}
                onChange={(e) => setForm((prev) => ({ ...prev, providerType: e.target.value as ProviderType }))}
                style={{ width: '100%', padding: '0.5rem', background: 'var(--bg-card)', border: '1px solid var(--border-color)', color: 'var(--text-primary)', borderRadius: '4px' }}
              >
                <option value="openAiCompatible">OpenAI Compatible</option>
                <option value="anthropic">Anthropic</option>
              </select>
            </div>
            <div style={{ gridColumn: 'span 2' }}>
              <label htmlFor="provider-endpoint">Endpoint</label>
              <input
                id="provider-endpoint"
                value={form.endpoint}
                onChange={(e) => setForm((prev) => ({ ...prev, endpoint: e.target.value }))}
                style={{ width: '100%' }}
              />
            </div>
            <div style={{ gridColumn: 'span 2' }}>
              <label htmlFor="provider-api-key">API Key</label>
              <input
                id="provider-api-key"
                value={form.apiKey}
                onChange={(e) => setForm((prev) => ({ ...prev, apiKey: e.target.value }))}
                style={{ width: '100%' }}
              />
            </div>

            <div style={{ gridColumn: 'span 2' }}>
              <label htmlFor="provider-model-input">模型列表</label>
              <div style={{ display: 'flex', gap: 8 }}>
                <input
                  id="provider-model-input"
                  value={form.modelInput}
                  onChange={(e) => setForm((prev) => ({ ...prev, modelInput: e.target.value }))}
                  placeholder="输入模型名后添加"
                  style={{ width: '100%' }}
                />
                <button type="button" className="btn btn-ghost" onClick={handleAddModel}>添加</button>
                <button type="button" className="btn btn-ghost" onClick={() => void handleFetchModels()} disabled={isFetchingModels || !form.endpoint.trim()}>
                  <RefreshCw size={14} /> 拉取
                </button>
              </div>
              <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap', marginTop: 10 }}>
                {form.models.map((item) => (
                  <span key={item} className="badge badge-purple" style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}>
                    {item}
                    <button
                      type="button"
                      className="btn btn-ghost btn-xs"
                      onClick={() => handleRemoveModel(item)}
                      style={{ padding: 0, minWidth: 'unset' }}
                      aria-label={`删除模型 ${item}`}
                    >
                      <X size={12} />
                    </button>
                  </span>
                ))}
              </div>
            </div>

            <div style={{ gridColumn: 'span 2' }}>
              <label htmlFor="provider-default-model">默认模型</label>
              <select
                id="provider-default-model"
                value={form.defaultModel}
                onChange={(e) => setForm((prev) => ({ ...prev, defaultModel: e.target.value }))}
                style={{ width: '100%', padding: '0.5rem', background: 'var(--bg-card)', border: '1px solid var(--border-color)', color: 'var(--text-primary)', borderRadius: '4px' }}
              >
                {form.models.length === 0 && <option value="">请先添加模型</option>}
                {form.models.map((item) => (
                  <option key={item} value={item}>{item}</option>
                ))}
              </select>
            </div>
          </div>
          <div style={{ display: 'flex', gap: 10, marginTop: 16 }}>
            <button type="button" className="btn btn-primary" onClick={handleSaveProvider}>
              {editingId ? '更新 Provider' : '保存 Provider'}
            </button>
          </div>
        </div>
      )}

      <div className="card-grid">
        {providers.map((p) => (
          <div key={p.id} className="card card-glow">
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 16 }}>
              <h4 style={{ fontSize: 16, fontWeight: 700 }}>{p.name}</h4>
              <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                {p.enabled
                  ? <span className="badge badge-green"><CheckCircle2 size={12} /> 可用</span>
                  : <span className="badge badge-red">未激活</span>
                }
                <button type="button" className="btn btn-ghost btn-sm" onClick={() => handleEditProvider(p)} title="编辑 Provider" aria-label="编辑 Provider">
                  <Pencil size={14} />
                </button>
                <button type="button" className="btn btn-ghost btn-sm" onClick={() => setDeleteTarget(p)} title="删除 Provider" aria-label="删除 Provider">
                  <Trash2 size={14} />
                </button>
              </div>
            </div>
            <div style={{ fontSize: 12, color: 'var(--text-tertiary)', fontFamily: 'var(--font-mono)', marginBottom: 12 }}>
              <Globe size={12} style={{ verticalAlign: 'middle', marginRight: 4 }} />
              {p.endpoint}
            </div>
            <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap', marginBottom: 12 }}>
              {(p.models.length > 0 ? p.models : [p.model]).map((m) => (
                <span key={m} className="badge badge-purple">{m}{m === p.model ? ' · 默认' : ''}</span>
              ))}
            </div>
            <div style={{ fontSize: 12, color: p.keySet ? 'var(--accent-green-light)' : 'var(--text-tertiary)' }}>
              <Key size={12} style={{ verticalAlign: 'middle', marginRight: 4 }} />
              {p.keySet ? 'API Key 已配置' : '未设置 API Key'}
            </div>
          </div>
        ))}
      </div>

      <ConfirmDialog
        open={Boolean(deleteTarget)}
        title="删除 Provider"
        description={deleteTarget ? `确认删除“${deleteTarget.name}”？此操作不可恢复。` : ''}
        confirmLabel="删除 Provider"
        cancelLabel="取消"
        danger
        onConfirm={() => void handleConfirmDelete()}
        onCancel={() => setDeleteTarget(null)}
      />
    </div>
  );
}
