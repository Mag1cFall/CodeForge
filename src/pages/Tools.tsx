import { useState, useEffect, useCallback } from 'react';
import { Wrench, Play, FileCode, Search, Terminal, BarChart3, Minimize2, type LucideIcon } from 'lucide-react';
import { useAppPreferences } from '../lib/app-preferences';
import { toolList, toolExecute, ToolSchema } from '../lib/backend';
import './PageCommon.css';

interface ToolView extends ToolSchema {
  category: string;
  calls: number;
  icon: LucideIcon;
}

const defaultArgsByTool: Record<string, string> = {
  read_file: '{\n  "path": "src-tauri/src/lib.rs"\n}',
  list_directory: '{\n  "path": "."\n}',
  search_code: '{\n  "query": "provider_list",\n  "path": "src-tauri/src"\n}',
  grep_pattern: '{\n  "pattern": "session_list",\n  "path": "src-tauri/src"\n}',
  run_shell: '{\n  "command": "cargo check",\n  "workdir": "src-tauri",\n  "timeoutSecs": 60\n}',
  run_tests: '{\n  "command": "cargo test -- --list",\n  "workdir": "src-tauri",\n  "timeoutSecs": 60\n}',
  write_file: '{\n  "path": ".codeforge-tool-test/sample.txt",\n  "content": "hello"\n}',
  apply_patch: '{\n  "path": ".codeforge-tool-test/sample.txt",\n  "old": "hello",\n  "new": "hello world"\n}',
  analyze_ast: '{\n  "path": "src/pages/Chat.tsx"\n}',
  check_complexity: '{\n  "path": "src/pages/Chat.tsx"\n}',
  find_code_smells: '{\n  "path": "src/pages/Chat.tsx"\n}',
  suggest_refactor: '{\n  "path": "src/pages/Chat.tsx"\n}',
};

const getDefaultArgsText = (toolName: string) => {
  return defaultArgsByTool[toolName] ?? '{\n  "path": "."\n}';
};

const classifyTool = (name: string): { category: string; icon: LucideIcon } => {
  if (['read_file', 'write_file', 'list_directory', 'apply_patch'].includes(name)) {
    return { category: '文件', icon: FileCode };
  }
  if (['search_code', 'grep_pattern'].includes(name)) {
    return { category: '搜索', icon: Search };
  }
  if (name === 'run_shell') {
    return { category: '执行', icon: Terminal };
  }
  if (['analyze_ast', 'check_complexity'].includes(name)) {
    return { category: '分析', icon: BarChart3 };
  }
  if (['find_code_smells', 'suggest_refactor'].includes(name)) {
    return { category: '审查', icon: Wrench };
  }
  return { category: '工具', icon: Wrench };
};

export default function Tools() {
  const { t } = useAppPreferences();
  const [tools, setTools] = useState<ToolView[]>([]);
  const [testingTool, setTestingTool] = useState<string | null>(null);
  const [testResults, setTestResults] = useState<Record<string, string>>({});
  const [testArgs, setTestArgs] = useState<Record<string, string>>({});

  const loadTools = useCallback(async () => {
    try {
      const data = await toolList();
      const next = (data ?? []).map((item) => {
        const meta = classifyTool(item.name);
        return {
          ...item,
          category: meta.category,
          calls: 0,
          icon: meta.icon,
        };
      });
      setTools(next);
    } catch {
      setTools([]);
    }
  }, []);

  useEffect(() => {
    void loadTools();
  }, [loadTools]);

  const runTest = async (toolName: string) => {
    const argsText = testArgs[toolName] ?? getDefaultArgsText(toolName);
    setTestResults((prev) => ({ ...prev, [toolName]: 'Running...' }));

    try {
      const parsed = JSON.parse(argsText) as Record<string, unknown>;
      if (toolName === 'apply_patch') {
        await toolExecute('write_file', {
          path: '.codeforge-tool-test/sample.txt',
          content: 'hello',
        });
      }
      const output = await toolExecute(toolName, parsed);
      setTestResults((prev) => ({ ...prev, [toolName]: output }));
    } catch {
      setTestResults((prev) => ({
        ...prev,
        [toolName]: '{\n  "status": "error",\n  "output": "Tool execution failed."\n}',
      }));
    }
  };

  return (
    <div className="animate-in">
      <div className="page-header">
        <h1><Wrench size={28} style={{ verticalAlign: 'middle', marginRight: 8 }} /> {t('route.tools')}</h1>
        <p>{t('page.tools.desc')}</p>
      </div>

      <div className="card-grid">
        {tools.map((tool) => (
          <div key={tool.name} className="card card-glow tool-card">
            <div className="tool-card-header">
              <div className="tool-icon"><tool.icon size={20} /></div>
              <span className="badge badge-blue">{tool.category}</span>
            </div>
            <h4 style={{ fontFamily: 'var(--font-mono)', fontSize: 14, marginBottom: 6 }}>{tool.name}</h4>
            <p className="text-secondary" style={{ fontSize: 13, marginBottom: 12 }}>{tool.description}</p>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <span style={{ fontSize: 12, color: 'var(--text-tertiary)' }}>调用 {tool.calls} 次</span>
              <button className="btn btn-sm btn-secondary" onClick={() => setTestingTool(tool.name === testingTool ? null : tool.name)}>
                {testingTool === tool.name ? <Minimize2 size={14} /> : <Play size={14} />} {testingTool === tool.name ? '收起' : '测试'}
              </button>
            </div>
            {testingTool === tool.name && (
              <div style={{ marginTop: 16, padding: 12, background: 'var(--bg-card)', borderRadius: 6, border: '1px solid var(--border-color)' }}>
                <div style={{ fontSize: 12, color: 'var(--text-tertiary)', marginBottom: 8 }}>参数 (JSON)</div>
                <textarea
                  style={{ width: '100%', height: 60, fontFamily: 'var(--font-mono)', fontSize: 12, background: 'var(--bg-main)', color: 'var(--text-primary)', border: '1px solid var(--border-color)', padding: 8, borderRadius: 4, resize: 'vertical' }}
                  value={testArgs[tool.name] ?? getDefaultArgsText(tool.name)}
                  onChange={(e) => {
                    setTestArgs((prev) => ({ ...prev, [tool.name]: e.target.value }));
                  }}
                />
                <button className="btn btn-sm btn-primary" style={{ marginTop: 8 }} onClick={() => void runTest(tool.name)}><Play size={12}/> 执行测试</button>
                {testResults[tool.name] && (
                  <pre style={{ marginTop: 8, padding: 8, background: 'var(--bg-main)', fontSize: 12, borderRadius: 4, color: 'var(--accent-green-light)', whiteSpace: 'pre-wrap' }}>
                    {testResults[tool.name]}
                  </pre>
                )}
              </div>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}
