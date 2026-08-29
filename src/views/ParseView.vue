<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from 'vue'

import { NButton, NCheckbox, NEmpty, NIcon, NInput, NPagination, NSpin, useMessage } from 'naive-ui'
import { CloudDownloadOutline, SearchOutline, TimeOutline, TrashOutline, CopyOutline } from '@vicons/ionicons5'

import ViewHeader from '@/components/layout/ViewHeader.vue'
import { useTransferStore } from '@/stores/transfer'
import { usePreferenceStore } from '@/stores/preference'
import { lanzouShareFolder, lanzouShareInfo } from '@/shared/api'
import { getFileIconColor, getFileIconComponent } from '@/shared/fileIcons'
import { expandRangeSelection } from '@/shared/util'
import { useRoute } from 'vue-router'
import { skipClipboardCheck } from '@/composables/useClipboardCheck'

interface ParseFile {
  name: string
  size: string
  time: string
  url: string
  pwd?: string
}

interface ParseRecord {
  title: string
  url: string
  pwd: string
  time: number
}

const HISTORY_KEY = 'lanzou.parseHistory'
const message = useMessage()
const transferStore = useTransferStore()
const preferenceStore = usePreferenceStore()
const route = useRoute()

const url = ref('')
const pwd = ref('')
const loading = ref(false)
const files = ref<ParseFile[]>([])
const selected = ref<ParseFile[]>([])
/** Shift 连选锚点：最近一次普通勾选项在当页列表中的索引 */
const selectAnchor = ref<number | null>(null)
/** 分享页标题（文件夹名/文件名），显示在工具栏左侧 */
const shareTitle = ref('')
// 搜索
const searchQuery = ref('')
const searchDebounceTimer = ref<ReturnType<typeof setTimeout> | null>(null)
// 解析记录
const showHistory = ref(false)
const history = ref<ParseRecord[]>([])
const historySelected = ref<string[]>([])
const historySearchQuery = ref('')

const filteredHistory = computed(() => {
  const q = historySearchQuery.value.toLowerCase()
  if (!q) return history.value
  return history.value.filter((r) => r.title.toLowerCase().includes(q) || r.url.toLowerCase().includes(q) || r.pwd?.toLowerCase().includes(q))
})

// 解析记录分页
const historyPage = ref(1)
const HISTORY_PAGE_SIZE = 20
const historyTotalPages = computed(() => Math.max(1, Math.ceil(filteredHistory.value.length / HISTORY_PAGE_SIZE)))
const pageHistory = computed(() => {
  const start = (historyPage.value - 1) * HISTORY_PAGE_SIZE
  return filteredHistory.value.slice(start, start + HISTORY_PAGE_SIZE)
})
watch(filteredHistory, () => { historyPage.value = 1 })

function loadHistory() {
  try {
    const raw = localStorage.getItem(HISTORY_KEY)
    history.value = raw ? JSON.parse(raw) : []
  } catch {
    history.value = []
  }
}

function saveHistory(record: ParseRecord) {
  // 去重：相同 URL 只保留最新一条
  const filtered = history.value.filter((r) => r.url !== record.url)
  filtered.unshift(record)
  const limit = preferenceStore.config.parseHistoryLimit ?? 50
  if (limit > 0 && filtered.length > limit) filtered.length = limit
  history.value = filtered
  localStorage.setItem(HISTORY_KEY, JSON.stringify(filtered))
}

function removeHistory(url: string) {
  history.value = history.value.filter((r) => r.url !== url)
  historySelected.value = historySelected.value.filter((u) => u !== url)
  localStorage.setItem(HISTORY_KEY, JSON.stringify(history.value))
}

function toggleHistorySelect(url: string) {
  const idx = historySelected.value.indexOf(url)
  if (idx >= 0) historySelected.value.splice(idx, 1)
  else historySelected.value.push(url)
}

function isHistorySelected(url: string) {
  return historySelected.value.includes(url)
}

const allHistorySelected = computed(() => filteredHistory.value.length > 0 && filteredHistory.value.every((r) => isHistorySelected(r.url)))

function toggleSelectAllHistory() {
  if (allHistorySelected.value) {
    historySelected.value = []
  } else {
    historySelected.value = filteredHistory.value.map((r) => r.url)
  }
}

function deleteSelectedHistory() {
  if (!historySelected.value.length) return
  history.value = history.value.filter((r) => !historySelected.value.includes(r.url))
  localStorage.setItem(HISTORY_KEY, JSON.stringify(history.value))
  historySelected.value = []
}

function formatRecord(record: ParseRecord) {
  return record.pwd ? `${record.title} ${record.url} 密码:${record.pwd}` : `${record.title} ${record.url}`
}

async function copyHistory(record: ParseRecord) {
  try {
    await navigator.clipboard.writeText(formatRecord(record))
    skipClipboardCheck()
    message.success('已复制')
  } catch {
    message.error('复制失败')
  }
}

async function copySelectedHistory() {
  if (!historySelected.value.length) return
  const text = history.value
    .filter((r) => historySelected.value.includes(r.url))
    .map(formatRecord)
    .join('\n')
  try {
    await navigator.clipboard.writeText(text)
    skipClipboardCheck()
    message.success(`已复制 ${historySelected.value.length} 条`)
  } catch {
    message.error('复制失败')
  }
}

function viewHistory(record: ParseRecord) {
  url.value = record.url
  pwd.value = record.pwd
  showHistory.value = false
  void parse()
}

function onSearchInput(value: string) {
  if (searchDebounceTimer.value) clearTimeout(searchDebounceTimer.value)
  searchDebounceTimer.value = setTimeout(() => {
    searchQuery.value = value.trim()
    page.value = 1
  }, 150)
}

function clearSearch() {
  searchQuery.value = ''
  page.value = 1
}

// 列表滚动容器（测量滚动条宽度，与文件页表头对齐）
const scrollEl = ref<HTMLElement | null>(null)
function syncScrollbarGutter() {
  const el = scrollEl.value
  if (!el) return
  const sbw = el.offsetWidth - el.clientWidth
  if (sbw >= 0) document.documentElement.style.setProperty('--scrollbar-w', `${sbw}px`)
}
onMounted(() => {
  nextTick(syncScrollbarGutter)
  loadHistory()
  // 从路由查询参数填充（剪贴板跳转）
  const qUrl = route.query.url
  const qPwd = route.query.pwd
  if (qUrl && typeof qUrl === 'string') {
    url.value = qUrl
    if (qPwd && typeof qPwd === 'string') pwd.value = qPwd
    void parse()
  }
})

// 分页
const PAGE_SIZE = 20
const page = ref(1)
const filteredFiles = computed(() => {
  const q = searchQuery.value.toLowerCase()
  if (!q) return files.value
  return files.value.filter((f) => f.name.toLowerCase().includes(q))
})
const totalPages = computed(() => Math.max(1, Math.ceil(filteredFiles.value.length / PAGE_SIZE)))
const pageFiles = computed(() => {
  const start = (page.value - 1) * PAGE_SIZE
  return filteredFiles.value.slice(start, start + PAGE_SIZE)
})
watch(files, async () => {
  page.value = 1
  selected.value = []
  selectAnchor.value = null
  // 列表渲染后重新测量滚动条宽度，保证表头右三列与内容对齐
  await nextTick()
  syncScrollbarGutter()
})

// 全选基于当前页（与文件页一致）
const allSelected = computed(() => pageFiles.value.length > 0 && pageFiles.value.every((f) => isSelected(f)))

function toggleSelect(f: ParseFile) {
  const idx = selected.value.findIndex((s) => s.url === f.url)
  if (idx >= 0) selected.value.splice(idx, 1)
  else selected.value.push(f)
}

function isSelected(f: ParseFile) {
  return selected.value.some((s) => s.url === f.url)
}

/** 复选框点击：Shift 时按锚点区间连选；普通点击更新锚点 */
function onCheckClick(f: ParseFile, event: MouseEvent) {
  if (event.shiftKey && selectAnchor.value != null) {
    const cur = pageFiles.value.findIndex((x) => x.url === f.url)
    if (cur >= 0) {
      selected.value = expandRangeSelection(selected.value, pageFiles.value, selectAnchor.value, cur)
      return
    }
  }
  toggleSelect(f)
  selectAnchor.value = pageFiles.value.findIndex((x) => x.url === f.url)
}

function toggleSelectAll() {
  const pageItems = pageFiles.value
  if (allSelected.value) {
    // 取消当前页选中
    const pageUrls = new Set(pageItems.map((f) => f.url))
    selected.value = selected.value.filter((s) => !pageUrls.has(s.url))
  } else {
    for (const f of pageItems) {
      if (!isSelected(f)) selected.value.push(f)
    }
  }
}

/** 从粘贴文本中提取链接与密码 */
function extractFromPaste(text: string): { url: string; pwd: string } | null {
  // 移除换行
  const clean = text.replace(/\r?\n/g, ' ').trim()
  const urlMatch = clean.match(/https?:\/\/\S+/)
  if (!urlMatch) return null
  const url = urlMatch[0].replace(/[，。,\s]+$/, '')
  const after = clean.slice((urlMatch.index || 0) + urlMatch[0].length).trim()
  // 优先 "密码:xxx" / "密码：xxx"，否则取链接后的独立词（如 "链接 MCPF"）
  let pwd = ''
  const pwdMatch = after.match(/密码[:：]\s*([^\s，。、]+)/)
  if (pwdMatch) {
    pwd = pwdMatch[1]
  } else if (after) {
    pwd = after.split(/[\s，。、]+/)[0].replace(/[，。]$/, '')
  }
  return { url, pwd }
}

async function parse() {
  const inputUrl = url.value.trim()
  if (!inputUrl) {
    message.warning('请输入分享链接')
    return
  }
  loading.value = true
  files.value = []
  shareTitle.value = ''
  try {
    const share = await lanzouShareInfo(inputUrl, pwd.value || undefined)
    shareTitle.value = share.name
    if (share.type === 'file') {
      files.value = [{ name: share.name, size: '', time: '', url: inputUrl, pwd: pwd.value || share.pwd }]
    } else {
      const folder = await lanzouShareFolder(inputUrl, pwd.value || undefined)
      files.value = folder.list.map((f) => ({
        name: f.name,
        size: f.size,
        time: f.time,
        url: f.url,
        pwd: pwd.value || undefined,
      }))
    }
    if (!files.value.length) {
      message.info('未解析到文件')
    } else {
      saveHistory({ title: share.name, url: inputUrl, pwd: pwd.value || '', time: Date.now() })
    }
  } catch (e) {
    message.error(e instanceof Error ? e.message : String(e))
  } finally {
    loading.value = false
  }
}

/** 粘贴后回车：提取合法链接与密码填入输入框，再解析，并将焦点移出输入框 */
async function handleEnter() {
  const extracted = extractFromPaste(url.value)
  if (extracted) {
    url.value = extracted.url
    if (extracted.pwd) pwd.value = extracted.pwd
  }
  await parse()
  // 取消输入框焦点
  ;(document.activeElement as HTMLElement | null)?.blur?.()
}

function addToDownloads(list: ParseFile[]) {
  if (!list.length) return
  for (const f of list) {
    const id = `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`
    transferStore.addDownload({
      id,
      name: f.name,
      kind: 'download',
      status: 'pending',
      uploaded: 0,
      total: 0,
      speed: 0,
      url: f.url,
      pwd: f.pwd,
    })
  }
  transferStore.startAll('download')
  message.success(`已添加 ${list.length} 项到下载列表`)
}

function downloadSelected() {
  addToDownloads(selected.value.length ? selected.value : pageFiles.value)
}
</script>

<template>
  <div class="parse-view">
    <ViewHeader title="解析 URL" />
    <div class="parse-body">
      <div class="parse-form">
        <div class="parse-input-row">
          <NInput
            v-model:value="url"
            placeholder="粘贴分享文本，回车自动填充链接与密码"
            clearable
            @keydown.enter.prevent="handleEnter"
          />
          <NInput
            v-model:value="pwd"
            placeholder="密码（可选）"
            clearable
            style="width: 120px"
            @keydown.enter.prevent="handleEnter"
          />
          <NButton title="解析" type="primary" :loading="loading" @click="parse" :disabled="!url.trim()">
            <template #icon>
              <NIcon>
                <SearchOutline />
              </NIcon>
            </template>
          </NButton>
          <NButton title="解析记录" :class="{ 'history-active': showHistory }" @click="showHistory = !showHistory">
            <template #icon>
              <NIcon>
                <TimeOutline />
              </NIcon>
            </template>
          </NButton>
        </div>
      </div>

      <!-- 解析记录 / 解析文件列表共用区域 -->
      <div class="parse-list-wrap">
        <!-- 解析记录 -->
        <template v-if="showHistory">
          <div class="parse-bar">
            <span class="parse-count">解析记录</span>
            <div class="parse-bar-actions">
              <NInput
                v-model:value="historySearchQuery"
                placeholder="搜索记录..."
                size="small"
                clearable
                :disabled="!history.length"
                style="width: 300px"
              >
                <template #prefix>
                  <NIcon :size="16"><SearchOutline /></NIcon>
                </template>
              </NInput>
              <NButton size="small" :disabled="!historySelected.length" @click="copySelectedHistory">
                <template #icon><NIcon><CopyOutline /></NIcon></template>
                复制 ({{ historySelected.length }})
              </NButton>
              <NButton size="small" type="error" :disabled="!historySelected.length" @click="deleteSelectedHistory">
                <template #icon><NIcon><TrashOutline /></NIcon></template>
                删除 ({{ historySelected.length }})
              </NButton>
            </div>
          </div>
          <div v-if="!history.length" class="parse-empty">
            <NEmpty description="暂无记录" />
          </div>
          <template v-else>
            <div v-if="!filteredHistory.length" class="parse-empty">
              <NEmpty description="无匹配记录" />
            </div>
            <div v-else class="parse-table">
              <div class="file-row file-head">
                <span class="col-check" @click.stop="toggleSelectAllHistory">
                  <NCheckbox :checked="allHistorySelected" />
                </span>
                <span class="col-name">
                  <template v-if="historySelected.length > 0">
                    <span class="selection-info">已选择{{ historySelected.length }}项 </span>
                    <span class="selection-deselect" @click.stop="historySelected = []">取消选择</span>
                  </template>
                  <template v-else>标题</template>
                </span>
                <span class="col-link">链接</span>
                <span class="col-pwd">密码</span>
                <span class="col-action">操作</span>
              </div>
              <div class="parse-scroll">
                <div v-for="record in pageHistory" :key="record.url" class="file-row" :class="{ selected: isHistorySelected(record.url) }">
                  <span class="col-check" @click.stop="toggleHistorySelect(record.url)">
                    <NCheckbox :checked="isHistorySelected(record.url)" />
                  </span>
                  <span class="col-name">
                    <NIcon class="file-icon" :size="18" color="var(--m3-primary)">
                      <TimeOutline />
                    </NIcon>
                    <span class="file-name" :title="record.title">{{ record.title }}</span>
                  </span>
                  <span class="col-link" :title="record.url">{{ record.url }}</span>
                  <span class="col-pwd">{{ record.pwd || '—' }}</span>
                  <span class="col-action" @click.stop>
                    <NButton size="small" text title="查看" @click="viewHistory(record)">
                      <NIcon :size="20"><SearchOutline /></NIcon>
                    </NButton>
                    <NButton size="small" text title="复制链接" @click="copyHistory(record)">
                      <NIcon :size="20"><CopyOutline /></NIcon>
                    </NButton>
                    <NButton size="small" text type="error" title="删除" @click="removeHistory(record.url)">
                      <NIcon :size="20"><TrashOutline /></NIcon>
                    </NButton>
                  </span>
                </div>
              </div>
            </div>
            <div v-if="historyTotalPages > 1" class="files-pager">
              <NPagination v-model:page="historyPage" :page-count="historyTotalPages" />
            </div>
          </template>
        </template>

        <!-- 文件列表 -->
        <template v-else>
          <div class="parse-bar">
            <span class="parse-count" :title="shareTitle">{{ shareTitle || '文件列表' }}</span>
            <div class="parse-bar-actions">
              <NInput
                v-model:value="searchQuery"
                @update:value="onSearchInput"
                placeholder="搜索文件名..."
                size="small"
                clearable
                :disabled="!files.length"
                @clear="clearSearch"
                style="width: 300px"
              >
                <template #prefix>
                  <NIcon :size="16">
                    <SearchOutline />
                  </NIcon>
                </template>
              </NInput>
              <NButton size="small" type="primary" :disabled="!selected.length" @click="downloadSelected">
                <template #icon>
                  <NIcon><CloudDownloadOutline /></NIcon>
                </template>
                下载 ({{ selected.length }})
              </NButton>
            </div>
          </div>
          <NSpin :show="loading">
            <div v-if="!loading && files.length === 0" class="parse-empty">
              <NEmpty description="解析后显示文件列表" />
            </div>
            <div v-else class="parse-table">
              <div class="file-row file-head">
                <span class="col-check" @click.stop="toggleSelectAll">
                  <NCheckbox :checked="allSelected" />
                </span>
                <span class="col-name">
                  <template v-if="selected.length > 0">
                    <span class="selection-info">已选择{{ selected.length }}项 </span>
                    <span class="selection-deselect" @click.stop="selected = []">取消选择</span>
                  </template>
                  <template v-else>文件名</template>
                </span>
                <span class="col-size">大小</span>
                <span class="col-time">时间</span>
                <span class="col-action">操作</span>
              </div>
              <div class="parse-scroll" ref="scrollEl">
                <div v-for="f in pageFiles" :key="f.url" class="file-row" :class="{ selected: isSelected(f) }">
                  <span class="col-check" @click.stop="onCheckClick(f, $event)">
                    <NCheckbox :checked="isSelected(f)" />
                  </span>
                  <span class="col-name">
                    <NIcon class="file-icon" :size="18" :color="getFileIconColor(f.name)">
                      <component :is="getFileIconComponent(f.name)" />
                    </NIcon>
                    <span class="file-name" :title="f.name">{{ f.name }}</span>
                  </span>
                  <span class="col-size">{{ f.size || '—' }}</span>
                  <span class="col-time">{{ f.time || '—' }}</span>
                  <span class="col-action" @click.stop>
                    <NButton size="small" text title="下载" @click="addToDownloads([f])">
                      <NIcon :size="20">
                        <CloudDownloadOutline />
                      </NIcon>
                    </NButton>
                  </span>
                </div>
              </div>
            </div>
          </NSpin>
          <div v-if="totalPages > 1" class="files-pager">
            <NPagination v-model:page="page" :page-count="totalPages" />
          </div>
        </template>
      </div>
    </div>
  </div>
</template>

<style scoped>
.parse-body {
  padding-top: 8px;
  gap: 12px;
}

.parse-form {
  flex-shrink: 0;
}

.parse-input-row {
  display: flex;
  gap: 8px;
  align-items: center;
}

.parse-input-row :deep(.n-input:first-child) {
  flex: 1;
}

.parse-input-row :deep(.n-button.history-active) {
  background: var(--m3-surface-container-highest);
}

.parse-bar {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.parse-count {
  padding-left: 8px;
  font-size: 13px;
  color: var(--m3-on-surface-variant);
}

.parse-bar-actions {
  display: flex;
  gap: 8px;
}

.parse-list-wrap {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.parse-list-wrap :deep(.n-spin-container),
.parse-list-wrap :deep(.n-spin-content) {
  height: 100%;
  display: flex;
  flex-direction: column;
  min-height: 0;
}

.col-size {
  width: 110px;
}

.col-time {
  width: 130px;
}

.col-link {
  width: 270px;
  font-size: 14px;
  color: var(--m3-on-surface-variant);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex-shrink: 0;
}

.col-pwd {
  width: 110px;
  font-size: 14px;
  color: var(--m3-on-surface-variant);
  flex-shrink: 0;
}

.file-head :deep(.col-link),
.file-head :deep(.col-pwd) {
  font-size: 12px;
  color: inherit;
}

.col-action {
  width: 76px;
  display: flex;
  align-items: center;
  justify-content: flex-start;
  gap: 4px;
  flex-shrink: 0;
}
</style>
