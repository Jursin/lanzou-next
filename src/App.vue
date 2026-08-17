<script setup lang="ts">
import { onMounted, onUnmounted } from 'vue'
import { NConfigProvider, NMessageProvider, NDialogProvider, NNotificationProvider } from 'naive-ui'

import { useNaiveTheme } from '@/composables/useNaiveTheme'
import { usePreferenceStore } from '@/stores/preference'
import UpdateCheck from '@/components/UpdateCheck.vue'

const { theme, themeOverrides } = useNaiveTheme()
const preferenceStore = usePreferenceStore()

// 开发者模式关闭时阻止原生右键菜单；开启后保持正常浏览器行为。
function onContextMenu(e: MouseEvent) {
  if (!preferenceStore.config.developerMode) {
    e.preventDefault()
  }
}

/** 开发者模式关闭时抑制 F12 等调出开发者工具 */
function onKeyDown(e: KeyboardEvent) {
  if (preferenceStore.config.developerMode) return
  const isF12 = e.key === 'F12'
  const isCtrlShift = e.ctrlKey && e.shiftKey && ['I', 'J', 'C'].includes(e.key.toUpperCase())
  if (isF12 || isCtrlShift) {
    e.preventDefault()
  }
}

onMounted(async () => {
  window.addEventListener('contextmenu', onContextMenu)
  window.addEventListener('keydown', onKeyDown)
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
