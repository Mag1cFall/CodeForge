import { useState, KeyboardEvent } from 'react';
import { BookOpen, Plus, FileCode, Database, Search, Loader2 } from 'lucide-react';
import './PageCommon.css';

const repos = [
  { name: 'codeforge', path: 'C:\\Users\\l\\Desktop\\数据挖掘\\codeforge', files: 42, chunks: 356, status: 'indexed' },
  { name: 'astrbot', path: 'C:\\Users\\l\\Desktop\\数据挖掘\\ref\\astrbot', files: 187, chunks: 1240, status: 'indexing' },
];

export default function Knowledge() {
  const [repoList, setRepoList] = useState(repos);
  const [showForm, setShowForm] = useState(false);
  const [repoPath, setRepoPath] = useState('');
  const [searchQuery, setSearchQuery] = useState('');
  const [searchResults, setSearchResults] = useState<string[]>([]);

  const handleAddRepo = () => {
    if (repoPath) {
      setRepoList([...repoList, { name: repoPath.split(/[\/\\]/).pop() || 'New Repo', path: repoPath, files: 0, chunks: 0, status: 'indexing' }]);
      setShowForm(false);
      setRepoPath('');
    }
  };

  const handleSearch = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter' && searchQuery.trim()) {
      setSearchResults([
        `[mock] Found match in src/main.rs: ${searchQuery}`,
        `[mock] Related concepts in handlers.rs for ${searchQuery}`
      ]);
    }
  };

  return (
    <div className="animate-in">
      <div className="page-header">
        <h1><BookOpen size={28} style={{ verticalAlign: 'middle', marginRight: 8 }} /> 知识库 (RAG)</h1>
        <p>对代码仓库建立向量索引，实现语义级代码理解</p>
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
          <div key={repo.name} className="card card-glow">
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
                {repo.status === 'indexed'
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
