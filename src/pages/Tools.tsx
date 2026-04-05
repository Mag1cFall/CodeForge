import { Wrench, Play, FileCode, Search, Terminal, BarChart3 } from 'lucide-react';
import './PageCommon.css';

const tools = [
  { name: 'read_file', category: '文件', desc: '读取文件内容', calls: 342, icon: FileCode },
  { name: 'search_code', category: '搜索', desc: '在代码库中搜索', calls: 189, icon: Search },
  { name: 'run_shell', category: '执行', desc: '执行 shell 命令', calls: 87, icon: Terminal },
  { name: 'analyze_ast', category: '分析', desc: 'AST 语法树分析', calls: 156, icon: BarChart3 },
  { name: 'find_code_smells', category: '审查', desc: '代码异味检测', calls: 234, icon: Search },
  { name: 'suggest_refactor', category: '审查', desc: '重构建议生成', calls: 78, icon: Wrench },
];

export default function Tools() {
  return (
    <div className="animate-in">
      <div className="page-header">
        <h1>🔧 工具注册</h1>
        <p>管理 Agent 可调用的工具，查看调用日志和 JSON Schema</p>
      </div>

      <div className="card-grid">
        {tools.map((tool) => (
          <div key={tool.name} className="card card-glow tool-card">
            <div className="tool-card-header">
              <div className="tool-icon"><tool.icon size={20} /></div>
              <span className="badge badge-blue">{tool.category}</span>
            </div>
            <h4 style={{ fontFamily: 'var(--font-mono)', fontSize: 14, marginBottom: 6 }}>{tool.name}</h4>
            <p className="text-secondary" style={{ fontSize: 13, marginBottom: 12 }}>{tool.desc}</p>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <span style={{ fontSize: 12, color: 'var(--text-tertiary)' }}>调用 {tool.calls} 次</span>
              <button className="btn btn-sm btn-secondary"><Play size={14} /> 测试</button>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
