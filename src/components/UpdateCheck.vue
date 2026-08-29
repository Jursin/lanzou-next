<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { NIcon, NModal, NProgress, NTag } from 'naive-ui'
import { ArrowUpCircleOutline, AlertCircleOutline } from '@vicons/ionicons5'
import { useRouter } from 'vue-router'

import { useUpdateCheck, setDialogFn } from '@/composables/useUpdateCheck'
import { useClipboardCheck } from '@/composables/useClipboardCheck'
import type { UpdateInfo } from '@/shared/types'
import { VERSION } from '@/shared/constants'
import { formatSize } from '@/shared/util'

const { silentStartupCheck, startDownload, downloading, downloadProgress, downloadError, handleClose } =
  useUpdateCheck()

const router = useRouter()
useClipboardCheck(router)

const dialogVisible = ref(false)
const dialogInfo = ref<UpdateInfo | null>(null)

type Phase = 'available' | 'downloading' | 'error'

const phase = computed<Phase>(() => {
  if (downloading.value) return 'downloading'
  if (downloadError.value) return 'error'
  return 'available'
})

const dialogTitle = computed(() => {
  if (phase.value === 'error') return '更新失败'
  if (phase.value === 'downloading') return '正在更新'
  return '发现新版本'
})

const dialogIcon = computed(() => {
  if (phase.value === 'error') return AlertCircleOutline
  return ArrowUpCircleOutline
})

const positiveText = computed(() => {
  if (phase.value === 'downloading') return '取消下载'
  if (phase.value === 'error') return '重试'
  return '立即更新'
})

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
  downloadError.value = ''
  dialogInfo.value = info
  dialogVisible.value = true
}

async function onPositive() {
  if (phase.value === 'downloading') {
    await handleClose()
    dialogInfo.value = null
    dialogVisible.value = false
  } else if (dialogInfo.value) {
    await startDownload(dialogInfo.value)
  }
  return false
}

async function onClose() {
  dialogInfo.value = null
  dialogVisible.value = false
}

onMounted(() => {
  setDialogFn(showDialog)
  void silentStartupCheck()
})

onUnmounted(() => {
  setDialogFn(() => { })
})
</script>

<template>
  <NModal :show="dialogVisible" :mask-closable="!downloading" :close-on-esc="!downloading" preset="dialog"
    :title="dialogTitle" :positive-text="positiveText" :negative-text="phase === 'downloading' ? undefined : '稍后再说'"
    transform-origin="center" @positive-click="onPositive" @negative-click="onClose"
    @update:show="(v) => { if (!v) onClose() }">
    <template #icon>
      <NIcon :component="dialogIcon" />
    </template>

    <!-- 版本比较 -->
    <template v-if="phase === 'available' && dialogInfo">
      <div class="update-version-info">
        <div class="update-version-tags">
          <span class="version-tag version-old">v{{ VERSION }}</span>
          <span class="version-arrow">→</span>
          <span class="version-tag version-new">v{{ dialogInfo.version }}</span>
          <NTag v-if="dialogInfo.isPrerelease" size="small" type="warning" :bordered="false" round>测试版</NTag>
          <NTag v-else size="small" type="success" :bordered="false" round>正式版</NTag>
        </div>
        <div v-if="dialogInfo.publishedAt" class="update-publish-date">
          发布于 {{ new Date(dialogInfo.publishedAt).toLocaleDateString() }}
        </div>
      </div>
    </template>

    <!-- 下载进度 -->
    <template v-else-if="phase === 'downloading'">
      <div class="update-progress-wrap">
        <NProgress
          type="line"
          indicator-placement="inside"
          :percentage="percent"
          color="var(--m3-primary)"
        />
        <span class="update-progress-meta">{{ progressText }}</span>
      </div>
    </template>

    <!-- 错误信息 -->
    <template v-else-if="phase === 'error'">
      <div class="update-error-detail">{{ downloadError }}</div>
    </template>
  </NModal>
</template>

<style scoped>
.update-version-info {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
  padding: 8px 0;
}

.update-version-tags {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
  justify-content: center;
}

.version-tag {
  font-size: 20px;
  font-weight: 600;
  padding: 2px 10px;
  border-radius: 10px;
}

.version-old {
  color: var(--m3-on-surface-variant);
  opacity: 0.7;
}

.version-new {
  color: var(--m3-primary);
  background: var(--m3-primary-container);
}

.version-arrow {
  font-size: 16px;
  opacity: 0.4;
}

.update-publish-date {
  font-size: 15px;
  color: var(--m3-on-surface-variant);
}

.update-progress-wrap {
  padding: 8px 0;
}

.update-progress-meta {
  display: block;
  text-align: right;
  font-size: 12px;
  color: var(--m3-on-surface-variant);
  margin-top: 6px;
}

.update-error-detail {
  background: var(--m3-error-container);
  color: var(--m3-on-error-container);
  border-radius: 8px;
  padding: 10px 14px;
  max-height: 80px;
  overflow-y: auto;
  font-size: 12.5px;
  word-break: break-all;
  line-height: 1.5;
}
</style>