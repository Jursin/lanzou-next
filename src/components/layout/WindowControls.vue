<script setup lang="ts">
/**
 * 自定义窗口控制按钮（最小化/最大化/关闭），用于无原生标题栏的窗口。
 */
import { onMounted, onUnmounted, ref } from 'vue'
import { getCurrentWindow } from '@tauri-apps/api/window'

const appWindow = ref<ReturnType<typeof getCurrentWindow> | null>(null)
const isMaximized = ref(false)
const isFocused = ref(true)
let unlisteners: Array<() => void> = []

onMounted(async () => {
  if (!('__TAURI_INTERNALS__' in window)) return
  try {
    const win = getCurrentWindow()
    appWindow.value = win
    isMaximized.value = await win.isMaximized()
    const un1 = await win.onResized(async () => {
      isMaximized.value = await win.isMaximized()
    })
    const un2 = await win.onFocusChanged(({ payload }) => {
      isFocused.value = payload
    })
    unlisteners = [un1, un2]
  } catch {
    /* 非 Tauri 环境忽略 */
  }
})

onUnmounted(() => {
  unlisteners.forEach((u) => u?.())
})

function minimize() {
  appWindow.value?.minimize()
}

function toggleMaximize() {
  appWindow.value?.toggleMaximize()
}

function close() {
  // 触发 CloseRequested：Rust 端按"关闭时最小化到托盘"配置决定隐藏/销毁或退出
  appWindow.value?.close()
}
</script>

<template>
  <div class="caption-bar" :class="{ unfocused: !isFocused }">
    <button class="caption-btn" title="最小化" @click="minimize">
      <svg width="10" height="1" viewBox="0 0 10 1" fill="none" aria-hidden="true">
        <path d="M0 .5h10" stroke="currentColor" stroke-width="1" />
      </svg>
    </button>
    <button class="caption-btn" :title="isMaximized ? '还原' : '最大化'" @click="toggleMaximize">
      <svg v-if="isMaximized" width="10" height="10" viewBox="0 0 10 10" fill="none" aria-hidden="true">
        <path d="M3.5 0.5h6v6M0.5 3.5h6v6h-6z" stroke="currentColor" stroke-width="1" />
      </svg>
      <svg v-else width="10" height="10" viewBox="0 0 10 10" fill="none" aria-hidden="true">
        <rect x="0.5" y="0.5" width="9" height="9" stroke="currentColor" stroke-width="1" />
      </svg>
    </button>
    <button class="caption-btn caption-close" title="关闭" @click="close">
      <svg width="10" height="10" viewBox="0 0 10 10" fill="none" aria-hidden="true">
        <path d="M0 0l10 10M10 0L0 10" stroke="currentColor" stroke-width="1.2" />
      </svg>
    </button>
  </div>
</template>

<style scoped>
.caption-bar {
  position: fixed;
  top: 0;
  right: 0;
  display: flex;
  height: 32px;
  z-index: 9999;
}
.caption-btn {
  width: 46px;
  height: 32px;
  border: none;
  border-radius: 0;
  background: transparent;
  color: var(--m3-on-surface);
  opacity: 0.7;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition:
    background-color 0.2s ease,
    opacity 0.2s ease;
  outline: none;
  padding: 0;
}
.caption-btn:hover {
  opacity: 1;
  background: color-mix(in srgb, var(--m3-on-surface) 8%, transparent);
}
.caption-btn:active {
  background: color-mix(in srgb, var(--m3-on-surface) 12%, transparent);
}
.caption-close:hover {
  background: #c42b1c;
  color: #fff;
  opacity: 1;
}
.caption-close:active {
  background: #b22a1b;
  color: #fff;
}
.unfocused .caption-btn {
  opacity: 0.4;
}
.unfocused .caption-btn:hover {
  opacity: 1;
}
</style>
