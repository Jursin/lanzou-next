<script setup lang="ts">
import { NIcon } from 'naive-ui'
import type { Component } from 'vue'

export interface SubnavPaneItem {
  key: string
  label: string
  icon: Component
  route: string
  active: boolean
}

defineProps<{
  title: string
  items: SubnavPaneItem[]
}>()

defineEmits<{ navigate: [route: string] }>()
</script>

<template>
  <aside class="subnav">
    <nav class="subnav-inner">
      <h3>{{ title }}</h3>
      <ul>
        <li v-for="item in items" :key="item.key">
          <button
            type="button"
            class="subnav-button"
            :class="{ active: item.active }"
            :aria-current="item.active ? 'page' : undefined"
            :aria-label="item.label"
            @click="$emit('navigate', item.route)"
          >
            <NIcon :size="16" class="subnav-icon">
              <component :is="item.icon" />
            </NIcon>
            <span class="subnav-label">{{ item.label }}</span>
          </button>
        </li>
      </ul>
    </nav>
  </aside>
</template>

<style scoped>
.subnav {
  width: 100%;
  height: 100%;
  background-color: var(--subnav-bg);
  color: var(--subnav-text);
  overflow-y: auto;
  overflow-x: hidden;
}
.subnav-inner {
  margin-top: var(--header-top-offset);
  padding: 0 24px;
  user-select: none;
}
.subnav-inner h3 {
  font-size: 16px;
  color: var(--subnav-title);
  font-weight: normal;
  line-height: 24px;
  margin: 0 0 20px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.subnav-inner ul {
  list-style: none;
  padding: 0;
  margin: 0;
  cursor: default;
}
.subnav-inner li {
  margin-bottom: 8px;
}
.subnav-button {
  position: relative;
  width: 100%;
  margin-bottom: 8px;
  padding: 8px 10px;
  font-size: 14px;
  line-height: 20px;
  border-radius: 6px;
  cursor: pointer;
  transition:
    background-color 0.3s cubic-bezier(0.2, 0, 0, 1),
    color 0.2s ease;
  display: flex;
  align-items: center;
  text-align: left;
  color: inherit;
  background: transparent;
  border: none;
  min-width: 0;
}
.subnav-button:hover,
.subnav-button.active,
.subnav-button:focus-visible {
  background-color: var(--subnav-active-bg);
  outline: none;
}
.subnav-button:hover span,
.subnav-button:hover .subnav-icon,
.subnav-button.active span,
.subnav-button.active .subnav-icon,
.subnav-button:focus-visible span,
.subnav-button:focus-visible .subnav-icon {
  color: var(--subnav-active-text);
}
.subnav-label {
  flex: 1 1 auto;
  min-width: 0;
  white-space: normal;
  overflow-wrap: anywhere;
}
.subnav-icon {
  flex: 0 0 auto;
  margin-right: 12px;
}
</style>
