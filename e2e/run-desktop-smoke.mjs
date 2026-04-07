import assert from 'node:assert/strict'
import fs from 'node:fs/promises'
import path from 'node:path'
import os from 'node:os'
import { fileURLToPath } from 'node:url'
import { spawn } from 'node:child_process'
import dotenv from 'dotenv'
import { Builder, By, Capabilities, Key, until } from 'selenium-webdriver'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const rootDir = path.resolve(__dirname, '..')
const applicationPath = path.join(rootDir, 'src-tauri', 'target', 'debug', 'codeforge.exe')
const tauriDriverPath = process.env.TAURI_DRIVER_PATH || path.join(os.homedir(), '.cargo', 'bin', 'tauri-driver.exe')

dotenv.config({ path: path.join(rootDir, '.env') })

async function main() {
  const fixtureDir = await createFixtureProject()
  const appDataDir = path.join(os.tmpdir(), `codeforge-e2e-data-${Date.now()}`)
  await fs.mkdir(appDataDir, { recursive: true })
  console.log(`[e2e] fixture=${fixtureDir}`)
  console.log(`[e2e] appData=${appDataDir}`)

  let tauriDriver
  let driver
  try {
    tauriDriver = spawn(tauriDriverPath, [], {
      cwd: rootDir,
      stdio: ['ignore', 'inherit', 'inherit'],
      env: {
        ...process.env,
        CODEFORGE_DATA_DIR: appDataDir,
      },
    })

    await waitForDriverServer('http://127.0.0.1:4444/status')

    const capabilities = new Capabilities()
    capabilities.setBrowserName('wry')
    capabilities.set('tauri:options', { application: applicationPath })

    driver = await new Builder()
      .usingServer('http://127.0.0.1:4444/')
      .withCapabilities(capabilities)
      .build()

    await driver.manage().setTimeouts({ implicit: 2000, pageLoad: 60000, script: 60000 })

    await expectText(driver, 'CodeForge')
    await testProviders(driver)
    await testSettings(driver, fixtureDir)
    await testChat(driver)
    await testReview(driver, fixtureDir)
    await testAgents(driver)
    await testMcp(driver)
    await testSkills(driver)
    await testKnowledge(driver, fixtureDir)
    await testLogs(driver)
    await testDashboard(driver)

    console.log('[e2e] desktop smoke ok')
  } catch (error) {
    await saveFailureArtifacts(driver, error)
    throw error
  } finally {
    if (driver) {
      await driver.quit().catch(() => {})
    }
    if (tauriDriver) {
      tauriDriver.kill()
    }
  }
}

async function saveFailureArtifacts(driver, error) {
  try {
    const outDir = path.join(rootDir, 'e2e', 'artifacts')
    await fs.mkdir(outDir, { recursive: true })
    if (driver) {
      const screenshot = await driver.takeScreenshot().catch(() => null)
      if (screenshot) {
        await fs.writeFile(path.join(outDir, 'failure.png'), screenshot, 'base64')
      }
      const body = await driver.findElement(By.css('body')).getText().catch(() => '')
      await fs.writeFile(path.join(outDir, 'failure.txt'), `${String(error)}\n\n${body}`)
    }
  } catch {}
}

async function testProviders(driver) {
  console.log('[e2e] providers')
  await clickNav(driver, '模型配置')
  await clickButton(driver, '添加 Provider')
  await typeInto(driver, By.id('provider-name'), 'e2e-provider')
  await typeInto(driver, By.id('provider-endpoint'), process.env.OPENAI_API_BASE || '')
  await typeInto(driver, By.id('provider-api-key'), process.env.OPENAI_API_KEY || '')
  await typeInto(driver, By.id('provider-model'), process.env.OPENAI_MODEL || 'gpt-5.4-mini(xhigh)')
  await clickButton(driver, '保存 Provider')
  await expectText(driver, 'e2e-provider')
}

async function testSettings(driver, fixtureDir) {
  console.log('[e2e] settings')
  await clickNav(driver, '设置')
  await typeByLabel(driver, '默认项目路径', fixtureDir)
  await clickButton(driver, '保存设置')
  await expectText(driver, '已保存')
}

async function testChat(driver) {
  console.log('[e2e] chat')
  await clickNav(driver, '对话')
  await createNewChatSession(driver)
  let baseline = await assistantMessageCount(driver)
  await sendChat(driver, '你好，请介绍一下你自己')
  await waitForAssistantResponse(driver, baseline)

  await createNewChatSession(driver)
  baseline = await assistantMessageCount(driver)
  await sendChat(driver, '请读取当前项目的 package.json 文件内容，并告诉我 name 字段。')
  const toolResponse = await waitForAssistantResponse(driver, baseline)
  assert.match(toolResponse, /fixture-project|package\.json/i)

  await createNewChatSession(driver)
  baseline = await assistantMessageCount(driver)
  await sendChat(driver, '你在哪个目录现在？列出一下文件')
  await expectText(driver, 'Agent 请求权限')
  await clickButton(driver, '允许执行')
  const resumed = await waitForAssistantResponse(driver, baseline)
  assert.match(resumed, /README|package\.json|当前目录|当前工作目录/i)
}

async function createNewChatSession(driver) {
  const button = await driver.findElement(By.xpath("//div[contains(@class,'chat-sidebar-header')][.//span[normalize-space(.)='会话历史']]//button"))
  await driver.executeScript('arguments[0].click();', button)
  await driver.sleep(500)
}

async function testReview(driver, fixtureDir) {
  console.log('[e2e] review')
  await clickNav(driver, '代码审查')
  await typeByPlaceholder(driver, '项目路径...', fixtureDir)
  await clickButton(driver, '下一步')
  await clickButton(driver, '开始审查')
  await expectText(driver, '问题列表', 90000)
}

async function testAgents(driver) {
  console.log('[e2e] agents')
  await clickNav(driver, 'Agent 管理')
  await clickButton(driver, '创建 Agent')
  await typeInto(driver, By.id('agent-name'), 'E2E Agent')
  await typeInto(driver, By.id('agent-instructions'), '用于桌面 UI 自动化测试')
  await typeInto(driver, By.id('agent-tools'), 'read_file,run_shell')
  await clickButton(driver, '保存当前 Agent')
  await expectText(driver, 'E2E Agent')
}

async function testMcp(driver) {
  console.log('[e2e] mcp')
  await clickNav(driver, 'MCP 服务')
  await clickButton(driver, '添加 MCP Server')
  await typeByLabel(driver, '名称', 'e2e-mcp')
  await typeByPlaceholder(driver, '例如: node /path/to/mcp/index.js', 'cmd')
  await clickButton(driver, '添加服务')
  await expectText(driver, 'e2e-mcp')
}

async function testSkills(driver) {
  console.log('[e2e] skills')
  await clickNav(driver, '技能市场')
  await expectText(driver, 'code-review')
}

async function testKnowledge(driver, fixtureDir) {
  console.log('[e2e] knowledge')
  const fixtureName = path.basename(fixtureDir)
  await clickNav(driver, '知识库')
  await clickButton(driver, '添加代码仓库')
  await typeByPlaceholder(driver, '本地路径 或 Git URL', fixtureDir)
  await clickButton(driver, '确认添加')
  await expectText(driver, fixtureName, 90000)
  const searchBox = await driver.findElement(By.xpath("//input[@placeholder='语义搜索代码 (按 Enter)...']"))
  await clearAndType(searchBox, 'Agent Loop')
  await searchBox.sendKeys(Key.ENTER)
  await expectText(driver, 'Agent Loop', 90000)
}

async function testLogs(driver) {
  console.log('[e2e] logs')
  await clickNav(driver, '执行日志')
  await expectText(driver, 'knowledge_index')
  await expectText(driver, 'knowledge_search')
}

async function testDashboard(driver) {
  console.log('[e2e] dashboard')
  await clickNav(driver, '仪表盘')
  const statCard = await driver.findElement(By.css('.stat-card'))
  await statCard.click()
  await expectText(driver, 'Agent 管理')
}

async function sendChat(driver, message) {
  const input = await driver.findElement(By.css('textarea.chat-input'))
  await clearAndType(input, message)
  const sendButton = await driver.findElement(By.css('button.chat-send-btn'))
  await sendButton.click()
}

async function waitForAssistantResponse(driver, previousCount = 0) {
  const deadline = Date.now() + 90000
  while (Date.now() < deadline) {
    await autoApprovePermissionIfPresent(driver)
    try {
      const contents = await driver.findElements(By.css('.chat-msg-assistant .chat-msg-content'))
      if (contents.length > previousCount) {
        const text = (await contents[contents.length - 1].getText()).trim()
        if (
          text.length > 0
          && !text.includes('Agent 正在思考')
          && text !== '等待权限确认后继续执行。'
        ) {
          return text
        }
      }
    } catch {}
    await driver.sleep(500)
  }

  throw new Error('waitForAssistantResponse timeout')
}

async function autoApprovePermissionIfPresent(driver) {
  try {
    const dialogs = await driver.findElements(By.xpath("//*[contains(normalize-space(.), 'Agent 请求权限') and contains(@class, 'perm-dialog')]") )
    if (dialogs.length === 0) return
    const buttons = await driver.findElements(By.xpath("//button[normalize-space(.)='允许执行' or .//*[normalize-space(text())='允许执行']]") )
    if (buttons.length === 0) return
    await driver.executeScript('arguments[0].click();', buttons[0])
    await driver.sleep(300)
  } catch {}
}

async function assistantMessageCount(driver) {
  const items = await driver.findElements(By.css('.chat-msg-assistant .chat-msg-content'))
  return items.length
}

async function clickNav(driver, label) {
  const pathMap = {
    '仪表盘': '/',
    '对话': '/chat',
    '代码审查': '/review',
    'Agent 管理': '/agents',
    '工具注册': '/tools',
    'MCP 服务': '/mcp',
    '技能市场': '/skills',
    '知识库': '/knowledge',
    '模型配置': '/providers',
    '执行日志': '/logs',
    '设置': '/settings',
  }
  const route = pathMap[label] || '/'
  const link = await driver.findElement(By.css(`a.nav-item[href='${route}']`))
  await driver.executeScript('arguments[0].click();', link)
}

async function clickButton(driver, label) {
  const button = await driver.findElement(By.xpath(`//button[normalize-space(.)='${label}' or .//*[normalize-space(text())='${label}']]`))
  await driver.executeScript('arguments[0].click();', button)
}

async function typeInto(driver, locator, value) {
  const element = await driver.findElement(locator)
  await clearAndType(element, value)
}

async function typeByLabel(driver, label, value) {
  const input = await driver.findElement(By.xpath(`//label[normalize-space(.)='${label}']/following-sibling::*[self::input or self::select or self::textarea][1]`))
  await clearAndType(input, value)
}

async function typeByPlaceholder(driver, placeholder, value) {
  const input = await driver.findElement(By.xpath(`//input[@placeholder='${placeholder}']`))
  await clearAndType(input, value)
}

async function clearAndType(element, value) {
  await element.clear().catch(() => {})
  await element.sendKeys(Key.chord(Key.CONTROL, 'a'), Key.BACK_SPACE)
  if (value) {
    await element.sendKeys(value)
  }
}

async function expectText(driver, text, timeout = 30000) {
  const locator = By.xpath(`//*[contains(normalize-space(.), '${text.replace(/'/g, "\'")}')]`)
  await driver.wait(until.elementLocated(locator), timeout)
  return driver.findElement(locator)
}

async function waitForDriverServer(url) {
  const start = Date.now()
  while (Date.now() - start < 30000) {
    try {
      const response = await fetch(url)
      if (response.ok) return
    } catch {}
    await new Promise((resolve) => setTimeout(resolve, 500))
  }
  throw new Error('tauri-driver did not start in time')
}

async function createFixtureProject() {
  const fixture = path.join(os.tmpdir(), `codeforge-e2e-${Date.now()}`)
  await fs.mkdir(path.join(fixture, 'src'), { recursive: true })
  await fs.writeFile(
    path.join(fixture, 'package.json'),
    JSON.stringify({ name: 'fixture-project', version: '1.0.0', scripts: { lint: 'echo lint' } }, null, 2),
  )
  await fs.writeFile(
    path.join(fixture, 'README.md'),
    '# Agent Loop\n\nThis fixture repository is used for desktop UI smoke tests.\n',
  )
  await fs.writeFile(
    path.join(fixture, 'src', 'main.rs'),
    'fn main() { let value = Some(1).unwrap(); println!("{}", value); panic!("demo panic"); }\n',
  )
  return fixture
}

main().catch((error) => {
  console.error('[e2e] saving failure artifacts')
  // artifacts best-effort; caller keeps non-zero exit code.
  console.error('[e2e] failure', error)
  process.exitCode = 1
})
