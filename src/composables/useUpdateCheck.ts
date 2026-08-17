import { ref } from 'vue'
import { listen } from '@tauri-apps/api/event'
import { useDialog, useMessage } from 'naive-ui'

import { checkForUpdate, downloadAndInstall } from '@/shared/api'
import type { UpdateInfo, UpdateDownloadProgress } from '@/shared/types'
import { usePreferenceStore } from '@/stores/preference'

/**
 * 更新检查：支持启动静默检查与手动检查。
 * 启动检查失败时静默忽略；手动检查会明确提示结果。
 */
export function useUpdateCheck() {
  const message = useMessage()
  const dialog = useDialog()
  const preferenceStore = usePreferenceStore()
  const downloading = ref(false)
  const downloadProgress = ref<UpdateDownloadProgress | null>(null)

  function recordCheckTime() {
    const now = Date.now()
    preferenceStore.config = { ...preferenceStore.config, lastCheckUpdateTime: now }
    void preferenceStore.update({ lastCheckUpdateTime: now })
  }

  function showUpdateDialog(info: UpdateInfo) {
    const hasAsset = !!info.assetUrl
    dialog.info({
      title: '发现新版本',
      content: `检测到新版本 v${info.version}${info.isPrerelease ? ' (测试版)' : ''}，是否立即更新？`,
      positiveText: hasAsset ? '立即更新' : '前往下载',
      negativeText: '暂不',
      transformOrigin: 'center',
      onPositiveClick: () => {
        if (hasAsset) {
          startDownloadAndInstall(info)
        } else {
          void import('@tauri-apps/plugin-opener').then(({ openUrl }) => openUrl(info.url))
        }
      },
    })
  }

  async function startDownloadAndInstall(info: UpdateInfo) {
    if (downloading.value) return
    downloading.value = true
    downloadProgress.value = null

    const unlisten = await listen<UpdateDownloadProgress>('update:download-progress', (event) => {
      downloadProgress.value = event.payload
    })

    try {
      await downloadAndInstall(info)
    } catch (e) {
      message.error(e instanceof Error ? e.message : String(e))
    } finally {
      unlisten()
      downloading.value = false
      downloadProgress.value = null
    }
  }

  /** 启动静默检查：仅在有新版本时弹窗 */
  async function silentStartupCheck() {
    if (preferenceStore.config.autoCheckUpdate === false) return
    try {
      const info = await checkForUpdate(!!preferenceStore.config.betaUpdate)
      recordCheckTime()
      if (info) showUpdateDialog(info)
    } catch {
      /* 占位接口不可用时静默忽略 */
    }
  }

  /** 手动检查：成功 / 失败 / 已是最新都提示 */
  async function manualCheck() {
    let info: UpdateInfo | null
    try {
      info = await checkForUpdate(!!preferenceStore.config.betaUpdate)
    } catch (e) {
      message.error(e instanceof Error ? e.message : String(e))
      return
    }
    recordCheckTime()
    if (info) {
      showUpdateDialog(info)
    } else {
      message.success('已是最新版本')
    }
  }

  return { silentStartupCheck, manualCheck, recordCheckTime, downloading, downloadProgress }
}
