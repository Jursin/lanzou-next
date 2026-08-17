import { ref } from 'vue'
import { listen } from '@tauri-apps/api/event'
import { useMessage } from 'naive-ui'

import { checkForUpdate, cancelDownload, downloadAndInstall } from '@/shared/api'
import type { UpdateInfo, UpdateDownloadProgress } from '@/shared/types'
import { usePreferenceStore } from '@/stores/preference'

const downloading = ref(false)
const downloadProgress = ref<UpdateDownloadProgress | null>(null)

let dialogFn: ((info: UpdateInfo) => void) | null = null

export function setDialogFn(fn: (info: UpdateInfo) => void) {
  dialogFn = fn
}

export function useUpdateCheck() {
  const message = useMessage()
  const preferenceStore = usePreferenceStore()

  function recordCheckTime() {
    const now = Date.now()
    preferenceStore.config = { ...preferenceStore.config, lastCheckUpdateTime: now }
    void preferenceStore.update({ lastCheckUpdateTime: now })
  }

  async function startDownload(info: UpdateInfo) {
    if (downloading.value) return
    downloading.value = true
    downloadProgress.value = null
    const unlisten = await listen<UpdateDownloadProgress>('update:download-progress', (event) => {
      downloadProgress.value = event.payload
    })
    try {
      await downloadAndInstall(info)
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e)
      if (msg !== '更新检查失败: 下载已取消') {
        message.error(msg)
      }
      downloading.value = false
      downloadProgress.value = null
    } finally {
      unlisten()
    }
  }

  async function handleClose() {
    if (downloading.value) {
      await cancelDownload()
      downloading.value = false
      downloadProgress.value = null
    }
  }

  async function silentStartupCheck() {
    if (preferenceStore.config.autoCheckUpdate === false) return
    try {
      const info = await checkForUpdate(!!preferenceStore.config.betaUpdate)
      recordCheckTime()
      if (info && dialogFn) dialogFn(info)
    } catch { /* ignore */ }
  }

  async function manualCheck() {
    let info: UpdateInfo | null
    try {
      info = await checkForUpdate(!!preferenceStore.config.betaUpdate)
    } catch (e) {
      message.error(e instanceof Error ? e.message : String(e))
      return
    }
    recordCheckTime()
    if (info && dialogFn) {
      dialogFn(info)
    } else if (!info) {
      message.success('已是最新版本')
    }
  }

  return { silentStartupCheck, manualCheck, downloading, downloadProgress, startDownload, handleClose }
}
