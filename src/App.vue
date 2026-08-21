<script setup lang="ts">
import { onMounted, onUnmounted } from 'vue'
import { NConfigProvider, NMessageProvider, NDialogProvider, NNotificationProvider } from 'naive-ui'

import { getCurrentWindow } from '@tauri-apps/api/window'
import { useNaiveTheme } from '@/composables/useNaiveTheme'
import { calibrateWindowSize } from '@/composables/useWindowSize'
import { usePreferenceStore } from '@/stores/preference'
import UpdateCheck from '@/components/UpdateCheck.vue'

const { theme, themeOverrides } = useNaiveTheme()
const preferenceStore = usePreferenceStore()

// 开发者工具关闭时阻止原生右键菜单；开启后保持正常浏览器行为。
function onContextMenu(e: MouseEvent) {
  if (!preferenceStore.config.devTools) {
    e.preventDefault()
  }
}

/** 开发者工具关闭时抑制 F12 等调出开发者工具 */
function onKeyDown(e: KeyboardEvent) {
  if (preferenceStore.config.devTools) return
  const isF12 = e.key === 'F12'
  const isCtrlShift = e.ctrlKey && e.shiftKey && ['I', 'J', 'C'].includes(e.key.toUpperCase())
  if (isF12 || isCtrlShift) {
    e.preventDefault()
  }
}

onMounted(async () => {
  window.addEventListener('contextmenu', onContextMenu)
  window.addEventListener('keydown', onKeyDown)
  // dev 模式下 Vite HMR 会因 setSize 触发页面重载导致黑屏，仅在生产环境校准尺寸
  if (!import.meta.env.DEV) {
    void calibrateWindowSize()
  } else if ('__TAURI_INTERNALS__' in window) {
    getCurrentWindow().show().catch(() => {})
  }
  try {
    await preferenceStore.load()
  } catch {
    /* ignore */
  }
})

onUnmounted(() => {
  window.removeEventListener('contextmenu', onContextMenu)
  window.removeEventListener('keydown', onKeyDown)
})
</script>

<template>
  <NConfigProvider :theme="theme" :theme-overrides="themeOverrides">
    <NNotificationProvider>
      <NMessageProvider>
        <NDialogProvider>
          <router-view />
          <UpdateCheck />
        </NDialogProvider>
      </NMessageProvider>
    </NNotificationProvider>
  </NConfigProvider>
</template>
