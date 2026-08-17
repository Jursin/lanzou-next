<script setup lang="ts">
import { computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { FolderOutline, CloudDownloadOutline, LinkOutline, SettingsOutline } from '@vicons/ionicons5'
import SubnavPane, { type SubnavPaneItem } from '@/components/layout/SubnavPane.vue'

const route = useRoute()
const router = useRouter()

const navItems: Omit<SubnavPaneItem, 'active'>[] = [
  { key: 'files', label: '我的文件', icon: FolderOutline, route: '/files' },
  { key: 'tasks', label: '传输列表', icon: CloudDownloadOutline, route: '/tasks' },
  { key: 'parse', label: '解析 URL', icon: LinkOutline, route: '/parse' },
  { key: 'setting', label: '设置', icon: SettingsOutline, route: '/setting' },
]

const items = computed<SubnavPaneItem[]>(() => navItems.map((item) => ({ ...item, active: route.path === item.route })))

function nav(to: string) {
  if (route.path !== to) router.push(to)
}
</script>

<template>
  <SubnavPane title="蓝奏云盘" :items="items" @navigate="nav" />
</template>
