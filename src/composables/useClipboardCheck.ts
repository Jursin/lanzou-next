import { h, onMounted, onUnmounted } from 'vue'
import { useDialog } from 'naive-ui'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { readText } from '@tauri-apps/plugin-clipboard-manager'
import type { Router } from 'vue-router'
import { extractShareLink } from '@/shared/share-link'

// 匹配蓝奏云分享链接（lanzn.com 及 lanzou*/lan[zs]o[ux] 旧域名）
const LANZOU_URL_RE =
  /https?:\/\/(?:[\w-]+\.)*(?:lanzn|lanzou[a-z0-9-]*|lan[zs]o[ux])\.(?:com|net|org|cn)\/\S+/i

let lastClipText = ''
let checking = false
let lastClipForDialog = ''
let skipNextCheck = false

// 标记下一次检测跳过（用于应用自身复制后避免重复弹窗）
export function skipClipboardCheck() {
  skipNextCheck = true
}

async function checkClipboard(router: Router, dialog: ReturnType<typeof useDialog>) {
  if (checking) return
  if (skipNextCheck) {
    skipNextCheck = false
    return
  }
  checking = true
  try {
    const text = await readText()
    if (!text || text === lastClipText) return
    lastClipText = text
    const info = extractShareLink(text, LANZOU_URL_RE)
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
