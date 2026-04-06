import { useState, KeyboardEvent, useEffect, useCallback } from 'react';
import { BookOpen, Plus, FileCode, Database, Search, Loader2 } from 'lucide-react';
import { useAppPreferences } from '../lib/app-preferences';
import { knowledgeRepos, knowledgeIndex, knowledgeSearch, projectOpen } from '../lib/backend';
import './PageCommon.css';

interface RepoItem {
  name: string;
  path: string;
  files: number;
  chunks: number;
  status: string;
}

const repoNameFromPath = (pathText: string): string => {
  const parts = pathText.split(/[\\/]/).filter((part) => part.length > 0);
  return parts[parts.length - 1] || 'repo';
};

export default function Knowledge() {
  const { t } = useAppPreferences();
  const [repoList, setRepoList] = useState<RepoItem[]>([]);
  const [showForm, setShowForm] = useState(false);
  const [repoPath, setRepoPath] = useState('');
  const [searchQuery, setSearchQuery] = useState('');
  const [searchResults, setSearchResults] = useState<string[]>([]);

  const loadRepos = useCallback(async () => {
    try {
      const repos = await knowledgeRepos();
      const next = await Promise.all((repos ?? []).map(async (repo) => {
        let fileCount = 0;
        try {
          const info = await projectOpen(repo.path);
          fileCount = info.fileCount;
        } catch {
          fileCount = 0;
        }

        return {
          name: repoNameFromPath(repo.path),
          path: repo.path,
          files: fileCount,
          chunks: repo.chunkCount,
          status: repo.status,
        };
      }));
      setRepoList(next);
    } catch {
      setRepoList([]);
    }
  }, []);

  useEffect(() => {
    void loadRepos();
  }, [loadRepos]);

  const handleAddRepo = () => {
    if (!repoPath) {
      return;
    }

    void (async () => {
      try {
        await knowledgeIndex(repoPath);
        await loadRepos();
      } catch {
        setRepoList([]);
      }

      setShowForm(false);
      setRepoPath('');
    })();
  };

  const handleSearch = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key !== 'Enter' || !searchQuery.trim()) {
      return;
    }

    void (async () => {
      try {
        const data = await knowledgeSearch(searchQuery.trim(), 5);
        const next = (data ?? []).map((item) => {
          const contentPreview = item.content.replace(/\s+/g, ' ').slice(0, 120);
          return `${item.filePath}: ${contentPreview}`;
        });
        setSearchResults(next);
      } catch {
        setSearchResults([]);
      }
    })();
  };

  return (
    <div className="animate-in">
      <div className="page-header">
        <h1><BookOpen size={28} style={{ verticalAlign: 'middle', marginRight: 8 }} /> {t('route.knowledge')}</h1>
        <p>{t('page.knowledge.desc')}</p>
      </div>

      <div className="page-toolbar">
        <button className="btn btn-primary" onClick={() => setShowForm(!showForm)}>
          <Plus size={16} /> {showForm ? '取消' : '添加代码仓库'}
        </button>
        <div className="search-box">
          <Search size={16} />
          <input
            placeholder="语义搜索代码 (按 Enter)..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            onKeyDown={handleSearch}
          />
        </div>
      </div>

      {showForm && (
        <div className="card" style={{ marginBottom: 20 }}>
          <div style={{ display: 'flex', gap: 8 }}>
            <input
              value={repoPath}
              onChange={e => setRepoPath(e.target.value)}
              placeholder="本地路径 或 Git URL"
              style={{ flex: 1 }}
            />
            <button className="btn btn-primary" onClick={handleAddRepo}>确认添加</button>
          </div>
        </div>
      )}

      {searchResults.length > 0 && (
        <div className="card" style={{ marginBottom: 20, background: 'var(--bg-card)' }}>
          <h3>搜索结果 🔍</h3>
          <ul style={{ paddingLeft: 20, marginTop: 10, color: 'var(--text-secondary)' }}>
            {searchResults.map((res, i) => <li key={i} style={{ marginBottom: 8 }}>{res}</li>)}
          </ul>
          <button className="btn btn-sm btn-ghost" onClick={() => setSearchResults([])}>清除结果</button>
        </div>
      )}

      <div style={{ display: 'flex', flexDirection: 'column', gap: 16, marginTop: 20 }}>
        {repoList.map((repo) => (
          <div key={repo.path} className="card card-glow">
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: 14 }}>
                <div style={{ width: 44, height: 44, borderRadius: 'var(--radius-md)', background: 'rgba(59, 130, 246, 0.1)', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
                  <Database size={22} color="var(--accent-blue-light)" />
                </div>
                <div>
                  <h4 style={{ fontSize: 15, fontWeight: 600 }}>{repo.name}</h4>
                  <span style={{ fontSize: 12, color: 'var(--text-tertiary)', fontFamily: 'var(--font-mono)' }}>{repo.path}</span>
                </div>
              </div>
              <div style={{ display: 'flex', alignItems: 'center', gap: 16 }}>
                <div style={{ textAlign: 'right' }}>
                  <div style={{ fontSize: 13 }}><FileCode size={14} style={{ verticalAlign: 'middle' }} /> {repo.files} 文件</div>
                  <div style={{ fontSize: 12, color: 'var(--text-tertiary)' }}>{repo.chunks} chunks</div>
                </div>
                {repo.status === 'indexed' || repo.status === 'ready'
                  ? <span className="badge badge-green">已索引</span>
                  : <span className="badge badge-orange"><Loader2 size={12} className="chat-typing-spinner" /> 索引中</span>
                }
              </div>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
