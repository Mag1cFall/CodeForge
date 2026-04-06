export type Locale = 'zh-CN' | 'zh-TW' | 'en';
export type ThemePreference = 'dark' | 'light' | 'auto';

export type TranslationKey =
  | 'sidebar.section.core'
  | 'sidebar.section.framework'
  | 'sidebar.section.data'
  | 'sidebar.section.system'
  | 'sidebar.toggle'
  | 'topbar.unknown'
  | 'topbar.search'
  | 'topbar.searchTitle'
  | 'topbar.notifications'
  | 'topbar.themeToggle'
  | 'route.dashboard'
  | 'route.chat'
  | 'route.review'
  | 'route.agents'
  | 'route.tools'
  | 'route.mcp'
  | 'route.skills'
  | 'route.knowledge'
  | 'route.providers'
  | 'route.logs'
  | 'route.settings'
  | 'page.dashboard.desc'
  | 'page.review.desc'
  | 'page.agents.desc'
  | 'page.tools.desc'
  | 'page.mcp.desc'
  | 'page.skills.desc'
  | 'page.knowledge.desc'
  | 'page.providers.desc'
  | 'page.logs.desc'
  | 'page.settings.desc'
  | 'settings.theme'
  | 'settings.language'
  | 'settings.theme.dark'
  | 'settings.theme.light'
  | 'settings.theme.auto'
  | 'settings.language.zh-CN'
  | 'settings.language.zh-TW'
  | 'settings.language.en';

export const LOCALE_STORAGE_KEY = 'language';
export const THEME_STORAGE_KEY = 'theme';
export const DEFAULT_LOCALE: Locale = 'zh-CN';
export const DEFAULT_THEME: ThemePreference = 'dark';

const translations: Record<Locale, Record<TranslationKey, string>> = {
  'zh-CN': {
    'sidebar.section.core': '核心', 'sidebar.section.framework': 'Agent 框架', 'sidebar.section.data': '数据', 'sidebar.section.system': '系统', 'sidebar.toggle': '收起侧栏',
    'topbar.unknown': '未知页面', 'topbar.search': '搜索', 'topbar.searchTitle': '全局搜索', 'topbar.notifications': '通知', 'topbar.themeToggle': '切换主题',
    'route.dashboard': '仪表盘', 'route.chat': '对话', 'route.review': '代码审查', 'route.agents': 'Agent 管理', 'route.tools': '工具注册', 'route.mcp': 'MCP 服务', 'route.skills': '技能市场', 'route.knowledge': '知识库', 'route.providers': '模型配置', 'route.logs': '执行日志', 'route.settings': '设置',
    'page.dashboard.desc': '基于多Agent协作的代码智能分析与最佳实践挖掘平台', 'page.review.desc': 'Agent 驱动的代码质量分析 · 沙箱隔离执行 · 完整审查报告输出', 'page.agents.desc': '配置和管理 AI Agent 角色、指令和工具权限', 'page.tools.desc': '管理 Agent 可调用的工具，查看调用日志和 JSON Schema', 'page.mcp.desc': '管理 Model Context Protocol 服务器，扩展 Agent 能力边界', 'page.skills.desc': 'Skill = Prompt + Tools + MCP — 赋予 Agent 专业领域能力', 'page.knowledge.desc': '对代码仓库建立向量索引，实现语义级代码理解', 'page.providers.desc': '配置 LLM Provider 连接，支持 OpenAI / Anthropic / DeepSeek / Ollama 等', 'page.logs.desc': 'Agent 执行 trace、工具调用日志、Token 消耗明细', 'page.settings.desc': '全局配置、主题、语言、项目路径',
    'settings.theme': '主题', 'settings.language': '语言', 'settings.theme.dark': '深色', 'settings.theme.light': '浅色', 'settings.theme.auto': '跟随系统', 'settings.language.zh-CN': '简体中文', 'settings.language.zh-TW': '繁體中文', 'settings.language.en': 'English',
  },
  'zh-TW': {
    'sidebar.section.core': '核心', 'sidebar.section.framework': 'Agent 框架', 'sidebar.section.data': '資料', 'sidebar.section.system': '系統', 'sidebar.toggle': '收合側欄',
    'topbar.unknown': '未知頁面', 'topbar.search': '搜尋', 'topbar.searchTitle': '全域搜尋', 'topbar.notifications': '通知', 'topbar.themeToggle': '切換主題',
    'route.dashboard': '儀表板', 'route.chat': '對話', 'route.review': '程式碼審查', 'route.agents': 'Agent 管理', 'route.tools': '工具註冊', 'route.mcp': 'MCP 服務', 'route.skills': '技能市場', 'route.knowledge': '知識庫', 'route.providers': '模型配置', 'route.logs': '執行日誌', 'route.settings': '設定',
    'page.dashboard.desc': '基於多 Agent 協作的程式碼智慧分析與最佳實務挖掘平台', 'page.review.desc': '由 Agent 驅動的程式碼品質分析、沙箱隔離執行與完整審查報告', 'page.agents.desc': '配置並管理 AI Agent 角色、指令與工具權限', 'page.tools.desc': '管理 Agent 可呼叫的工具，檢視呼叫日誌與 JSON Schema', 'page.mcp.desc': '管理 Model Context Protocol 伺服器，擴充 Agent 能力邊界', 'page.skills.desc': 'Skill = Prompt + Tools + MCP，為 Agent 提供專業能力', 'page.knowledge.desc': '為程式碼倉庫建立向量索引，實現語意層級理解', 'page.providers.desc': '配置 LLM Provider 連線，支援 OpenAI、Anthropic、DeepSeek、Ollama 等', 'page.logs.desc': 'Agent 執行 trace、工具呼叫日誌與 Token 消耗明細', 'page.settings.desc': '全域配置、主題、語言與專案路徑',
    'settings.theme': '主題', 'settings.language': '語言', 'settings.theme.dark': '深色', 'settings.theme.light': '淺色', 'settings.theme.auto': '跟隨系統', 'settings.language.zh-CN': '簡體中文', 'settings.language.zh-TW': '繁體中文', 'settings.language.en': 'English',
  },
  en: {
    'sidebar.section.core': 'Core', 'sidebar.section.framework': 'Agent Framework', 'sidebar.section.data': 'Data', 'sidebar.section.system': 'System', 'sidebar.toggle': 'Collapse sidebar',
    'topbar.unknown': 'Unknown Page', 'topbar.search': 'Search', 'topbar.searchTitle': 'Global Search', 'topbar.notifications': 'Notifications', 'topbar.themeToggle': 'Toggle theme',
    'route.dashboard': 'Dashboard', 'route.chat': 'Chat', 'route.review': 'Code Review', 'route.agents': 'Agents', 'route.tools': 'Tools', 'route.mcp': 'MCP Servers', 'route.skills': 'Skills', 'route.knowledge': 'Knowledge Base', 'route.providers': 'Providers', 'route.logs': 'Logs', 'route.settings': 'Settings',
    'page.dashboard.desc': 'A multi-agent platform for intelligent code analysis and best-practice discovery', 'page.review.desc': 'Agent-driven code quality analysis with sandboxed execution and full review reports', 'page.agents.desc': 'Configure and manage AI agent roles, instructions, and tool permissions', 'page.tools.desc': 'Manage callable tools, invocation logs, and JSON Schema definitions', 'page.mcp.desc': 'Manage Model Context Protocol servers and extend agent capabilities', 'page.skills.desc': 'Skill = Prompt + Tools + MCP for domain-specific agent abilities', 'page.knowledge.desc': 'Build vector indexes for repositories and enable semantic code understanding', 'page.providers.desc': 'Configure LLM providers including OpenAI, Anthropic, DeepSeek, and Ollama', 'page.logs.desc': 'Execution traces, tool call logs, and token usage details', 'page.settings.desc': 'Global preferences, theme, language, and project path',
    'settings.theme': 'Theme', 'settings.language': 'Language', 'settings.theme.dark': 'Dark', 'settings.theme.light': 'Light', 'settings.theme.auto': 'System', 'settings.language.zh-CN': 'Simplified Chinese', 'settings.language.zh-TW': 'Traditional Chinese', 'settings.language.en': 'English',
  },
};

export function normalizeLocale(value: string | null | undefined): Locale {
  if (value === 'zh-TW' || value === 'en') {
    return value;
  }
  return 'zh-CN';
}

export function normalizeTheme(value: string | null | undefined): ThemePreference {
  if (value === 'light' || value === 'auto') {
    return value;
  }
  return 'dark';
}

export function translate(locale: Locale, key: TranslationKey): string {
  return translations[locale][key] ?? translations[DEFAULT_LOCALE][key];
}
