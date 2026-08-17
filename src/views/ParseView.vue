<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from 'vue'

import { NButton, NCheckbox, NEmpty, NIcon, NInput, NPagination, NSpin, useMessage } from 'naive-ui'
import { CloudDownloadOutline, SearchOutline } from '@vicons/ionicons5'

import ViewHeader from '@/components/layout/ViewHeader.vue'
import { useTransferStore } from '@/stores/transfer'
import { lanzouShareFolder, lanzouShareInfo } from '@/shared/api'
import { getFileIconComponent, getFileIconInfo } from '@/shared/fileIcons'
import { expandRangeSelection } from '@/shared/util'

interface ParseFile {
  name: string
  size: string
  time: string
  url: string
  pwd?: string
}

const message = useMessage()
const transferStore = useTransferStore()

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

/** 从粘贴文本中提取链接与提取码 */
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
    if (!files.value.length) message.info('未解析到文件')
  } catch (e) {
    message.error(e instanceof Error ? e.message : String(e))
  } finally {
    loading.value = false
  }
}

/** 粘贴后回车：提取合法链接与提取码填入输入框，再解析，并将焦点移出输入框 */
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
        <NInput
          v-model:value="url"
          placeholder="粘贴分享文本，回车自动填充链接与提取码"
          @keydown.enter.prevent="handleEnter"
        />
        <div class="form-row">
          <NInput
            v-model:value="pwd"
            placeholder="提取码（可选）"
            style="width: 220px"
            @keydown.enter.prevent="handleEnter"
          />
          <NButton type="primary" :loading="loading" @click="parse">
            <template #icon
              ><NIcon><SearchOutline /></NIcon
            ></template>
            解析
          </NButton>
        </div>
      </div>

      <div v-if="files.length" class="parse-bar">
        <span class="parse-count" :title="shareTitle">{{ shareTitle || '文件列表' }}</span>
        <div class="parse-bar-actions">
          <NInput
            v-model:value="searchQuery"
            @update:value="onSearchInput"
            placeholder="搜索文件名..."
            size="small"
            clearable
            @clear="clearSearch"
            style="width: 250px"
          >
            <template #prefix>
              <NIcon :size="16"><SearchOutline /></NIcon>
            </template>
          </NInput>
          <NButton type="primary" size="small" :disabled="!selected.length" @click="downloadSelected">
            <template #icon
              ><NIcon><CloudDownloadOutline /></NIcon
            ></template>
            下载 ({{ selected.length }})
          </NButton>
        </div>
      </div>

      <div class="parse-list-wrap">
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
                文件名
                <span class="file-count">(共{{ filteredFiles.length }}项)</span>
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
                  <NIcon class="file-icon" :size="18" :color="getFileIconInfo(f.name).iconColor">
                    <component :is="getFileIconComponent(f.name)" />
                  </NIcon>
                  <span class="file-name" :title="f.name">{{ f.name }}</span>
                </span>
                <span class="col-size">{{ f.size || '—' }}</span>
                <span class="col-time">{{ f.time || '—' }}</span>
                <span class="col-action" @click.stop>
                  <NButton size="small" text title="下载" @click="addToDownloads([f])">
                    <NIcon :size="20"><CloudDownloadOutline /></NIcon>
                  </NButton>
                </span>
              </div>
            </div>
          </div>
        </NSpin>
        <div v-if="totalPages > 1" class="files-pager">
          <NPagination v-model:page="page" :page-count="totalPages" />
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.parse-view {
  height: 100%;
  display: flex;
  flex-direction: column;
}
.parse-body {
  flex: 1;
  margin: 0 36px 40px;
  display: flex;
  padding-top: 8px;
  flex-direction: column;
  gap: 12px;
  min-height: 0;
}
.parse-form {
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.form-row {
  display: flex;
  gap: 10px;
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
.parse-empty {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
}
.parse-table {
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
.parse-scroll {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  scrollbar-gutter: stable;
}
/* 文件行基础样式（hover/选中/表头/列）在 base.css 全局定义 */
.col-size {
  width: 110px;
  text-align: left;
  font-size: 12px;
  flex-shrink: 0;
  color: var(--m3-on-surface-variant);
}
.col-time {
  width: 130px;
  text-align: left;
  font-size: 12px;
  flex-shrink: 0;
  color: var(--m3-on-surface-variant);
}
.col-action {
  width: 48px;
  display: flex;
  align-items: center;
  justify-content: flex-start;
  flex-shrink: 0;
}
</style>
