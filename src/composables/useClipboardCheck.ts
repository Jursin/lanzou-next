import { h, onMounted, onUnmounted } from 'vue'
import { useDialog } from 'naive-ui'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { readText } from '@tauri-apps/plugin-clipboard-manager'
import type { Router } from 'vue-router'

/** 匹配蓝奏云分享链接 */
const LANZOU_URL_RE = /https?:\/\/[a-z0-9.-]*lanzn\.com\/\S+/i

/** 从文本中提取分享链接和密码 */
function extractShareInfo(text: string): { url: string; pwd: string } | null {
  const clean = text.replace(/\r?\n/g, ' ').trim()
  const urlMatch = clean.match(LANZOU_URL_RE)
  if (!urlMatch) return null
  const url = urlMatch[0].replace(/[，。,\s]+$/, '')
  const after = clean.slice((urlMatch.index || 0) + urlMatch[0].length).trim()
  let pwd = ''
  const pwdMatch = after.match(/密码[:：]\s*([^\s，。、]+)/)
  if (pwdMatch) {
    pwd = pwdMatch[1]
  } else if (after) {
    pwd = after.split(/[\s，。、]+/)[0].replace(/[，。]$/, '')
  }
  return { url, pwd }
}

let lastClipText = ''
let checking = false
let lastClipForDialog = ''

async function checkClipboard(router: Router, dialog: ReturnType<typeof useDialog>) {
  if (checking) return
  checking = true
  try {
    const text = await readText()
    if (!text || text === lastClipText) return
    lastClipText = text
    const info = extractShareInfo(text)
    if (!info) return
    lastClipForDialog = text
    dialog.info({
      title: '检测到分享链接',
      content: () => h('div', {
        style: 'font-size: 12px; color: var(--m3-on-surface-variant); background: var(--m3-surface-container-highest); padding: 8px; border-radius: 6px; word-break: break-all; max-height: 200px; overflow-y: auto;',
      }, lastClipForDialog),
      positiveText: '去解析',
      negativeText: '忽略',
      onPositiveClick: () => {
        router.push({ path: '/parse', query: { url: info.url, pwd: info.pwd } })
      },
    })
  } catch {
    // 剪贴板为空或读取失败，静默忽略
  } finally {
    checking = false
  }
}

export function useClipboardCheck(router: Router) {
  const dialog = useDialog()

  onMounted(() => {
    setTimeout(() => void checkClipboard(router, dialog), 1000)
    const unlisten = getCurrentWindow().onFocusChanged(({ payload: focused }) => {
      if (focused) {
        setTimeout(() => void checkClipboard(router, dialog), 300)
      }
    })
    onUnmounted(() => { unlisten.then((fn) => fn()) })
  })
}
