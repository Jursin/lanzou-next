<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { NModal, NTag } from 'naive-ui'

import { useUpdateCheck, setDialogFn } from '@/composables/useUpdateCheck'
import type { UpdateInfo } from '@/shared/types'
import { formatSize } from '@/shared/util'

const { silentStartupCheck, startDownload, downloading, downloadProgress, handleClose } = useUpdateCheck()

const dialogVisible = ref(false)
const dialogInfo = ref<UpdateInfo | null>(null)
const isDownloading = computed(() => downloading.value)

const percent = computed(() => {
  const p = downloadProgress.value
  if (!p || p.total === 0) return 0
  return Math.round((p.downloaded / p.total) * 100)
})

const progressText = computed(() => {
  const p = downloadProgress.value
  if (!p) return ''
  if (p.total === 0) return formatSize(p.downloaded)
  return `${formatSize(p.downloaded)} / ${formatSize(p.total)}`
})

function showDialog(info: UpdateInfo) {
  dialogInfo.value = info
  dialogVisible.value = true
}

async function onConfirm() {
  if (!dialogInfo.value) return
  await startDownload(dialogInfo.value)
}

async function onClose() {
  dialogInfo.value = null
  dialogVisible.value = false
  if (downloading.value) {
    await handleClose()
  }
}

onMounted(() => {
  setDialogFn(showDialog)
  void silentStartupCheck()
})

onUnmounted(() => {
  setDialogFn(() => {})
})
</script>

<template>
  <NModal
    :show="dialogVisible"
    :mask-closable="true"
    :close-on-esc="true"
    preset="dialog"
    :show-icon="false"
    :title="isDownloading ? '正在更新' : '发现新版本'"
    :positive-text="isDownloading ? undefined : '立即更新'"
    :negative-text="isDownloading ? undefined : '稍后再说'"
    transform-origin="center"
    @positive-click="onConfirm"
    @negative-click="onClose"
    @update:show="
      (v) => {
        if (!v) onClose()
      }
    "
  >
    <!-- 版本信息 -->
    <template v-if="!isDownloading && dialogInfo">
      <div style="display: flex; align-items: center; gap: 8px">
        <span style="font-size: 16px; font-weight: 600">v{{ dialogInfo.version }}</span>
        <NTag v-if="dialogInfo.isPrerelease" size="small" type="warning" :bordered="false" round>测试版</NTag>
        <NTag v-else size="small" type="success" :bordered="false" round>正式版</NTag>
      </div>
      <div v-if="dialogInfo.publishedAt" style="font-size: 12px; color: var(--m3-on-surface-variant); margin-top: 4px">
        发布于 {{ new Date(dialogInfo.publishedAt).toLocaleDateString() }}
      </div>
    </template>

    <!-- 下载进度 -->
    <template v-if="isDownloading">
      <div style="display: flex; justify-content: space-between; margin-bottom: 6px">
        <span style="font-size: 13px; color: var(--m3-on-surface-variant)">正在下载更新...</span>
        <span style="font-size: 13px; font-weight: 600; color: var(--m3-primary)">{{ percent }}%</span>
      </div>
      <div
        style="
          width: 100%;
          height: 4px;
          border-radius: 2px;
          background: var(--m3-surface-container-highest);
          overflow: hidden;
        "
      >
        <div
          :style="{
            width: percent + '%',
            height: '100%',
            borderRadius: '2px',
            background: 'var(--m3-primary)',
            transition: 'width 0.2s ease',
          }"
        />
      </div>
      <div style="font-size: 11px; color: var(--m3-on-surface-variant); text-align: right; margin-top: 4px">
        {{ progressText }}
      </div>
    </template>
  </NModal>
</template>
