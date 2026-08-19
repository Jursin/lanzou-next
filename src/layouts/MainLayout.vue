<script setup lang="ts">
import AppSubnav from '@/components/layout/AppSubnav.vue'
import WindowControls from '@/components/layout/WindowControls.vue'
</script>

<template>
  <div id="container">
    <!-- 无原生标题栏：顶部为拖拽区域，右上角为窗口控制按钮 -->
    <div class="titlebar-drag" data-tauri-drag-region></div>
    <WindowControls />
    <div class="subnav-slot">
      <AppSubnav />
    </div>
    <main class="content">
      <router-view />
    </main>
  </div>
</template>

<style scoped>
#container {
  display: flex;
  height: 100vh;
  position: relative;
  overflow: hidden;
}

.titlebar-drag {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  height: 32px;
  z-index: 1900;
  user-select: none;
  -webkit-app-region: drag;
}

.subnav-slot {
  width: var(--subnav-width);
  flex-shrink: 0;
  background-color: var(--subnav-bg);
}

.content {
  flex: 1;
  min-width: 0;
  overflow-y: auto;
  background-color: var(--main-bg);
}
</style>
