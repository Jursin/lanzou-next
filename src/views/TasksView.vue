<script setup lang="ts">
import { computed, h, onActivated, onMounted, onUnmounted, ref, watch } from 'vue'

import {
  NButton,
  NCheckbox,
  NEmpty,
  NDropdown,
  NIcon,
  NPagination,
  NProgress,
  NRadioGroup,
  NRadioButton,
  useDialog,
  useMessage,
  type DropdownOption,
} from 'naive-ui'
import { PauseOutline, PlayOutline, TrashOutline, OpenOutline, FolderOpenOutline } from '@vicons/ionicons5'
import { revealItemInDir, openPath } from '@tauri-apps/plugin-opener'

import ViewHeader from '@/components/layout/ViewHeader.vue'
import { useTransferStore, type TransferItem } from '@/stores/transfer'
import { lanzouCheckPath, lanzouDeleteLocal, lanzouDeleteLocalDir } from '@/shared/api'
import { formatSize } from '@/shared/util'

const transferStore = useTransferStore()
const message = useMessage()
const dialog = useDialog()

const tab = ref<'upload' | 'download'>('upload')
const stateFilter = ref<'all' | 'running' | 'paused' | 'done'>('all')
const selected = ref<TransferItem[]>([])
const contextItem = ref<TransferItem | null>(null)
const dropdownShow = ref(false)
const dropdownPos = ref({ x: 0, y: 0 })

function switchTab(value: 'upload' | 'download') {
  tab.value = value
  selected.value = []
  checkAllLost()
}

onMounted(() => {
  checkAllLost()
  // 周期 ticker：进行中任务的已耗时间持续刷新（进度事件间隔期间不冻结）
  tickerTimer = setInterval(() => {
    ticker.value = Date.now()
  }, 500)
})

onUnmounted(() => {
  if (tickerTimer) clearInterval(tickerTimer)
})

/** 周期刷新信号：elapsed/estimatedTotal 读取它以驱动重新渲染 */
const ticker = ref(0)
let tickerTimer: ReturnType<typeof setInterval> | undefined

// 切回本页（keep-alive 场景）或 tab 切换时重新检测丢失
onActivated(() => {
  checkAllLost()
})

const uploads = computed(() => transferStore.uploads)
const downloads = computed(() => transferStore.downloads)
const completed = computed(() => transferStore.completed)

// 状态计数
const uploadCount = computed(() => uploads.value.length)
const downloadCount = computed(() => downloads.value.length)
const allCount = computed(() => {
  const active = (tab.value === 'upload' ? uploads.value : downloads.value).length
  const done = tab.value === 'upload' ? completedUploads.value.length : completedDownloads.value.length
  return active + done
})
const runningCount = computed(
  () =>
    (tab.value === 'upload' ? uploads.value : downloads.value).filter(
      (i) => i.status === 'running' || i.status === 'pending' || i.status === 'error',
    ).length,
)
const pausedCount = computed(
  () => (tab.value === 'upload' ? uploads.value : downloads.value).filter((i) => i.status === 'paused').length,
)
const doneCount = computed(() => {
  const list = tab.value === 'upload' ? completedUploads.value : completedDownloads.value
  return list.length
})

// 当前页面的已完成项（按 kind 过滤）
const completedUploads = computed(() => completed.value.filter((i) => i.kind === 'upload'))
const completedDownloads = computed(() => completed.value.filter((i) => i.kind === 'download'))

// 当前 tab 的数据
const activeList = computed(() => {
  const list = tab.value === 'upload' ? uploads.value : downloads.value
  if (stateFilter.value === 'paused') return list.filter((i) => i.status === 'paused')
  if (stateFilter.value === 'done') return []
  if (stateFilter.value === 'all') return list.slice()
  return list.filter((i) => i.status === 'running' || i.status === 'pending' || i.status === 'error')
})
const activeCompleted = computed(() => {
  const list = tab.value === 'upload' ? completedUploads.value : completedDownloads.value
  if (stateFilter.value === 'done' || stateFilter.value === 'all') return list
  return []
})

// 分页（每页 20 项，已完成在前、进行中在后）
const TASKS_PAGE_SIZE = 20
const tasksPage = ref(1)
const totalTasksPages = computed(() => {
  const total = activeList.value.length + activeCompleted.value.length
  return Math.max(1, Math.ceil(total / TASKS_PAGE_SIZE))
})
const pageItems = computed(() => {
  const all = [...activeCompleted.value, ...activeList.value]
  const start = (tasksPage.value - 1) * TASKS_PAGE_SIZE
  return all.slice(start, start + TASKS_PAGE_SIZE)
})

watch([tab, stateFilter], () => {
  tasksPage.value = 1
})
watch(totalTasksPages, (tp) => {
  if (tasksPage.value > tp) tasksPage.value = tp
})

// 格式化
function fmtSpeed(speed: number) {
  return `${formatSize(speed)}/s`
}

function fmtDuration(ms: number) {
  if (!ms || ms < 0) return '—'
  const s = Math.max(1, Math.round(ms / 1000))
  const h = Math.floor(s / 3600)
  const m = Math.floor((s % 3600) / 60)
  const sec = s % 60
  let parts: string[] = []
  if (h > 0) parts.push(`${h}h`)
  if (m > 0) parts.push(`${m}m`)
  if (sec > 0 || parts.length === 0) parts.push(`${sec}s`)
  return parts.join('')
}

function elapsed(item: TransferItem) {
  void ticker.value
  const base = item.elapsedMs || 0
  if (!item.startedAt) return fmtDuration(base)
  const end = item.finishedAt || (item.status === 'running' ? Date.now() : item.startedAt)
  return fmtDuration(base + Math.max(0, end - item.startedAt))
}

function estimatedTotal(item: TransferItem) {
  void ticker.value
  if (item.status !== 'running' || item.speed <= 0 || item.total <= 0) return '—'
  // 预计总共需要时间 = 已用 + 剩余
  const left = item.total - item.uploaded
  const base = item.elapsedMs || 0
  const run = item.startedAt ? Math.max(0, Date.now() - item.startedAt) : 0
  const leftMs = (left / item.speed) * 1000
  return fmtDuration(base + run + leftMs)
}

function percent(item: TransferItem) {
  if (!item.total) return 0
  return Math.round((item.uploaded / item.total) * 100)
}

function hasKnownSize(item: TransferItem) {
  return item.total > 0
}

function statusLabel(item: TransferItem) {
  switch (item.status) {
    case 'pending':
      return '等待中'
    case 'running':
      return '进行中'
    case 'paused':
      return '已暂停'
    case 'done':
      return '已完成'
    case 'error':
      return item.error || '失败'
    default:
      return item.status
  }
}

// 选择
function toggleSelect(item: TransferItem) {
  const idx = selected.value.findIndex((s) => s.id === item.id)
  if (idx >= 0) selected.value.splice(idx, 1)
  else selected.value.push(item)
}

function isSelected(item: TransferItem) {
  return selected.value.some((s) => s.id === item.id)
}

function toggleSelectAll() {
  if (allSelected.value) {
    selected.value = []
  } else {
    selected.value = [...activeList.value, ...activeCompleted.value]
  }
}

const allSelected = computed(() => {
  const list = [...activeList.value, ...activeCompleted.value]
  return list.length > 0 && list.every((i) => selected.value.some((s) => s.id === i.id))
})

function confirmAction(title: string, content: string, onOk: () => void) {
  dialog.error({
    title,
    content,
    positiveText: '确定',
    negativeText: '取消',
    transformOrigin: 'center',
    onPositiveClick: onOk,
  })
}

// 删除文件选项：同时删除本地文件（默认不勾选）
const deleteLocalFile = ref(false)

function confirmDelete(title: string, content: string, onOk: (deleteLocal: boolean) => void, showLocal = false) {
  deleteLocalFile.value = false
  const checkboxRef = ref(false)
  dialog.error({
    title,
    positiveText: '删除',
    negativeText: '取消',
    transformOrigin: 'center',
    content: () =>
      h('div', { style: 'display:flex;flex-direction:column;gap:12px;' }, [
        h('div', null, content),
        showLocal
          ? h(
              NCheckbox,
              {
                checked: checkboxRef.value,
                'onUpdate:checked': (v: boolean) => {
                  checkboxRef.value = v
                  deleteLocalFile.value = v
                },
              },
              { default: () => '同时删除本地文件' },
            )
          : null,
      ]),
    onPositiveClick: () => onOk(deleteLocalFile.value),
  })
}

function deleteLocalFiles(items: TransferItem[]) {
  const dir = transferStore.downloadDir
  for (const item of items) {
    if (item.kind !== 'download') continue
    lanzouDeleteLocal(item.filePath || '', dir, item.name).catch(() => {})
    // 合并下载：同时清理本地 .parts 分片文件夹
    if (item.mergeFiles?.length) {
      lanzouDeleteLocalDir(dir, item.name).catch(() => {})
    }
  }
}

function deleteSelected() {
  const count = selected.value.length
  if (!count) return
  const items = selected.value.slice()
  const showLocal = items.some((i) => i.kind === 'download')
  confirmDelete(
    '删除',
    `确定删除选中的 ${count} 项吗？`,
    (deleteLocal) => {
      if (deleteLocal) deleteLocalFiles(items)
      for (const item of items) {
        if (item.status === 'done') {
          transferStore.removeCompleted(item.id)
        } else {
          transferStore.removeItem(item.kind, item.id)
        }
      }
      selected.value = []
    },
    showLocal,
  )
}

const hasStartable = computed(() =>
  selected.value.some((i) => i.status === 'paused' || i.status === 'error' || i.status === 'pending'),
)
const hasPausable = computed(() => selected.value.some((i) => i.status === 'running' || i.status === 'pending'))

function startSelected() {
  for (const item of selected.value) {
    if (item.status === 'paused' || item.status === 'error' || item.status === 'pending') {
      transferStore.startItem(item.kind, item.id)
    }
  }
}

function pauseSelected() {
  let hint = false
  for (const item of selected.value) {
    if (item.status === 'running' || item.status === 'pending') {
      transferStore.pauseItem(item.kind, item.id)
      if (item.kind === 'upload') hint = true
    }
  }
  if (hint) message.info('不支持上传断点续传，恢复上传后将重新上传')
}

function confirmRemoveItem(item: TransferItem) {
  confirmDelete(
    '删除',
    `确定删除“${item.name}”吗？`,
    (deleteLocal) => {
      if (deleteLocal) deleteLocalFiles([item])
      if (item.status === 'done') {
        transferStore.removeCompleted(item.id)
      } else {
        transferStore.removeItem(item.kind, item.id)
      }
    },
    item.kind === 'download',
  )
}

function confirmRetryItem(item: TransferItem) {
  confirmAction('重新下载', `确定重新下载“${item.name}”吗？`, () => {
    retryItem(item)
  })
}

// 单任务操作
function startItem(item: TransferItem) {
  transferStore.startItem(item.kind, item.id)
}
function pauseItem(item: TransferItem) {
  transferStore.pauseItem(item.kind, item.id)
  if (item.kind === 'upload') message.info('不支持上传断点续传，恢复上传后将重新上传')
}
function retryItem(item: TransferItem) {
  transferStore.retryItem(item.kind, item.id)
}

async function openFile(item: TransferItem) {
  if (!item.filePath) {
    message.warning('无法定位文件路径')
    return
  }
  try {
    await openPath(item.filePath)
  } catch (e) {
    message.error(e instanceof Error ? e.message : String(e))
  }
}

async function showInFolder(item: TransferItem) {
  if (!item.filePath) {
    message.warning('无法定位文件路径')
    return
  }
  try {
    await revealItemInDir(item.filePath)
  } catch (e) {
    message.error(e instanceof Error ? e.message : String(e))
  }
}

// 右键菜单：仅已完成下载项显示（打开文件/打开所在文件夹）；上传完成项无本地文件不显示打开
function onRowContext(item: TransferItem, event: MouseEvent) {
  if (item.status !== 'done' || item.kind !== 'download') return
  contextItem.value = item
  if (!isSelected(item)) {
    selected.value = [item]
  }
  dropdownPos.value = { x: event.clientX, y: event.clientY }
  dropdownShow.value = true
}

const contextOptions: DropdownOption[] = [
  { label: '打开文件', key: 'open', icon: () => h(NIcon, null, { default: () => h(OpenOutline) }) },
  { label: '打开所在文件夹', key: 'folder', icon: () => h(NIcon, null, { default: () => h(FolderOpenOutline) }) },
]

async function onContextSelect(key: string) {
  const item = contextItem.value
  if (!item) return
  dropdownShow.value = false
  if (key === 'open') await openFile(item)
  else if (key === 'folder') await showInFolder(item)
}

// 检查已完成下载文件是否仍存在（已丢失）
const lostMap = ref<Record<string, boolean>>({})

async function checkLost(item: TransferItem) {
  if (item.kind !== 'download' || !item.filePath) return
  try {
    const exists = await lanzouCheckPath(item.filePath)
    lostMap.value = { ...lostMap.value, [item.id]: !exists }
    transferStore.markLost(item.id, !exists)
  } catch {
    /* ignore */
  }
}

function checkAllLost() {
  completed.value.filter((i) => i.kind === 'download' && i.filePath).forEach((i) => checkLost(i))
}

watch(
  () => transferStore.completed.length,
  () => checkAllLost(),
)
</script>

<template>
  <div class="tasks-view">
    <ViewHeader title="传输列表">
      <NRadioGroup :value="tab" size="small" @update:value="switchTab">
        <NRadioButton value="upload">
          上传
          <span v-if="uploadCount" class="tasks-badge">{{ uploadCount }}</span>
        </NRadioButton>
        <NRadioButton value="download">
          下载
          <span v-if="downloadCount" class="tasks-badge">{{ downloadCount }}</span>
        </NRadioButton>
      </NRadioGroup>
    </ViewHeader>
    <div class="tasks-body">
      <!-- 工具栏：左侧全选 + 批量操作；右侧状态筛选 -->
      <div class="tasks-toolbar">
        <div class="toolbar-left">
          <span class="select-all" @click="toggleSelectAll">
            <NCheckbox :checked="allSelected" />
            <span class="select-all-label">全选</span>
          </span>
          <NButton size="small" :disabled="!hasStartable" @click="startSelected">
            <template #icon>
              <NIcon>
                <PlayOutline />
              </NIcon>
            </template>
            开始
          </NButton>
          <NButton size="small" :disabled="!hasPausable" @click="pauseSelected">
            <template #icon>
              <NIcon>
                <PauseOutline />
              </NIcon>
            </template>
            暂停
          </NButton>
          <NButton size="small" :disabled="!selected.length" @click="deleteSelected">
            <template #icon>
              <NIcon>
                <TrashOutline />
              </NIcon>
            </template>
            删除
          </NButton>
        </div>
        <div class="toolbar-right">
          <NRadioGroup :value="stateFilter" size="small" @update:value="(v: string) => stateFilter = v as typeof stateFilter">
            <NRadioButton value="all">
              全部
              <span v-if="allCount" class="tasks-badge">{{ allCount }}</span>
            </NRadioButton>
            <NRadioButton value="running">
              进行中
              <span v-if="runningCount" class="tasks-badge">{{ runningCount }}</span>
            </NRadioButton>
            <NRadioButton value="paused">
              已暂停
              <span v-if="pausedCount" class="tasks-badge">{{ pausedCount }}</span>
            </NRadioButton>
            <NRadioButton value="done">
              已完成
              <span v-if="doneCount" class="tasks-badge">{{ doneCount }}</span>
            </NRadioButton>
          </NRadioGroup>
        </div>
      </div>

      <div class="tasks-list">
        <NEmpty
          v-if="!activeList.length && !activeCompleted.length"
          :description="tab === 'upload' ? '暂无上传任务' : '暂无下载任务'"
          class="empty"
        />

        <!-- 任务项（已完成在前，进行中在后） -->
        <div
          v-for="item in pageItems"
          :key="`${item.status === 'done' ? 'done' : 'run'}-${item.id}`"
          class="task-item"
          :class="{ selected: isSelected(item) }"
          @click="toggleSelect(item)"
          @contextmenu.prevent="onRowContext(item, $event)"
        >
          <NCheckbox :checked="isSelected(item)" class="task-check" />
          <div class="task-main">
            <!-- 已完成 -->
            <template v-if="item.status === 'done'">
              <div class="task-top">
                <span class="task-name" :title="item.name">{{ item.name }}</span>
                <span v-if="lostMap[item.id]" class="task-status lost">已丢失</span>
                <span v-else class="task-status done">已完成</span>
              </div>
              <div class="task-meta">
                <span class="meta-item">{{ formatSize(item.total) }}</span>
                <span class="meta-item">{{ elapsed(item) }}</span>
              </div>
            </template>
            <!-- 进行中 -->
            <template v-else>
              <div class="task-top">
                <span class="task-name" :title="item.name">{{ item.name }}</span>
                <span class="task-status" :class="item.status">{{ statusLabel(item) }}</span>
              </div>
              <NProgress
                :percentage="percent(item)"
                :show-indicator="false"
                :height="6"
                :indeterminate="!hasKnownSize(item) && item.status === 'running'"
                class="task-progress"
              />
              <div class="task-meta">
                <template v-if="hasKnownSize(item)">
                  <span class="meta-item">{{ percent(item) }}%</span>
                  <span class="meta-item">{{ formatSize(item.uploaded) }} / {{ formatSize(item.total) }}</span>
                </template>
                <template v-else>
                  <span class="meta-item">{{ formatSize(item.uploaded) }}（大小未知）</span>
                </template>
                <span class="meta-item">{{ fmtSpeed(item.speed) }}</span>
                <span v-if="item.status === 'running' && hasKnownSize(item)" class="meta-item">
                  {{ elapsed(item) }}/{{ estimatedTotal(item) }}
                </span>
                <span v-else class="meta-item">{{ elapsed(item) }}</span>
              </div>
            </template>
          </div>
          <div class="task-actions" @click.stop>
            <!-- 已完成：重新下载（仅下载）+ 删除 -->
            <template v-if="item.status === 'done'">
              <NButton
                v-if="item.kind === 'download'"
                size="small"
                text
                title="重新下载"
                @click="confirmRetryItem(item)"
              >
                <NIcon :size="20">
                  <PlayOutline />
                </NIcon>
              </NButton>
              <NButton size="small" text title="删除" @click="confirmRemoveItem(item)">
                <NIcon :size="20">
                  <TrashOutline />
                </NIcon>
              </NButton>
            </template>
            <!-- 进行中：暂停/开始 + 删除 -->
            <template v-else>
              <NButton
                size="small"
                text
                :title="item.status === 'running' || item.status === 'pending' ? '暂停' : '开始'"
                @click="item.status === 'running' || item.status === 'pending' ? pauseItem(item) : startItem(item)"
              >
                <NIcon :size="20">
                  <PauseOutline v-if="item.status === 'running' || item.status === 'pending'" />
                  <PlayOutline v-else />
                </NIcon>
              </NButton>
              <NButton size="small" text title="删除" @click="confirmRemoveItem(item)">
                <NIcon :size="20">
                  <TrashOutline />
                </NIcon>
              </NButton>
            </template>
          </div>
        </div>

        <!-- 分页 -->
        <div v-if="totalTasksPages > 1" class="tasks-pager">
          <NPagination v-model:page="tasksPage" :page-count="totalTasksPages" />
        </div>
      </div>

      <!-- 右键菜单 -->
      <NDropdown
        v-model:show="dropdownShow"
        :options="contextOptions"
        :x="dropdownPos.x"
        :y="dropdownPos.y"
        @select="onContextSelect"
      />
    </div>
  </div>
</template>

<style scoped>
.tasks-badge {
  min-width: 16px;
  height: 16px;
  padding: 0 5px;
  border-radius: 8px;
  font-size: 11px;
  line-height: 16px;
  text-align: center;
  background: color-mix(in srgb, var(--m3-primary) 16%, transparent);
  color: var(--m3-primary);
}

.tasks-toolbar {
  flex-shrink: 0;
  display: flex;
  padding: 8px 0;
  justify-content: space-between;
  align-items: center;
  gap: 12px;
}

.toolbar-left {
  display: flex;
  align-items: center;
  gap: 12px;
}

.toolbar-right {
  display: flex;
  align-items: center;
  gap: 10px;
}

.select-all {
  display: flex;
  align-items: center;
  gap: 6px;
  cursor: pointer;
}

.select-all-label {
  font-size: 13px;
  color: var(--m3-on-surface-variant);
}

.tasks-list {
  flex: 1;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.empty {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
}

.task-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 12px;
  border-radius: 8px;
  background-color: var(--m3-surface-container-lowest);
  border: 1px solid var(--m3-outline-variant);
  cursor: pointer;
}

.task-item:hover {
  border-color: var(--m3-primary);
}

.task-item.selected {
  border-color: var(--m3-primary);
  background-color: color-mix(in srgb, var(--m3-primary) 8%, transparent);
}

.task-check {
  flex-shrink: 0;
}

.task-main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.task-top {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.task-name {
  flex: 1;
  font-size: 13px;
  color: var(--m3-on-surface);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.task-status {
  font-size: 12px;
  color: var(--m3-on-surface-variant);
  flex-shrink: 0;
}

.task-status.error {
  color: var(--m3-error);
}

.task-status.done {
  color: var(--m3-success);
}

.task-status.lost {
  color: var(--m3-error);
}

.task-status.paused {
  color: var(--m3-warning);
}

.task-status.running {
  color: var(--m3-primary);
}

.task-progress {
  width: 100%;
}

.task-meta {
  display: flex;
  gap: 14px;
  flex-wrap: wrap;
}

.meta-item {
  font-size: 12px;
  color: var(--m3-on-surface-variant);
}

.task-actions {
  flex-shrink: 0;
  display: flex;
  gap: 2px;
  opacity: 0.7;
}

.task-item:hover .task-actions {
  opacity: 1;
}
</style>
