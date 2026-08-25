<script setup lang="ts">
import { computed, h, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'

import {
  NButton,
  NCheckbox,
  NEmpty,
  NDropdown,
  NIcon,
  NInput,
  NModal,
  NPagination,
  NSpin,
  NSwitch,
  NText,
  NTree,
  NBreadcrumb,
  NBreadcrumbItem,
  useDialog,
  useMessage,
  type DropdownOption,
  type TreeOption,
} from 'naive-ui'
import {
  AddOutline,
  CaretUpOutline,
  CaretDownOutline,
  CloudDownloadOutline,
  CloudUploadOutline,
  DocumentsOutline,
  CreateOutline,
  DocumentOutline,
  DocumentTextOutline,
  FolderOutline,
  KeyOutline,
  LinkOutline,
  MoveOutline,
  RefreshOutline,
  SearchOutline,
  TrashOutline,
} from '@vicons/ionicons5'
import { open as openDialog } from '@tauri-apps/plugin-dialog'
import Sortable from 'sortablejs'
import type { SortableEvent } from 'sortablejs'
import QRCode from 'qrcode'

import ViewHeader from '@/components/layout/ViewHeader.vue'
import { useRecycleDelete, setRecycleDeleteFinish, setFileDeleteFinish } from '@/composables/useRecycleDelete'
import { useAppStore } from '@/stores/app'
import { useFilesStore } from '@/stores/files'
import { useTransferStore } from '@/stores/transfer'
import { usePreferenceStore } from '@/stores/preference'
import { useUploadTrafficStore } from '@/stores/uploadTraffic'
import {
  formatSize,
  expandRangeSelection,
  isPartName,
  parsePartIndex,
  commonMergedName,
  parseSizeText,
} from '@/shared/util'
import { getFileIconColor, getFileIconComponent } from '@/shared/fileIcons'
import type { LsFile, RecycleFile, RecycleItem } from '@/shared/types'
import {
  lanzouFileDescription,
  lanzouFileDetail,
  lanzouFolderDetail,
  lanzouLs,
  lanzouMkdir,
  lanzouMove,
  lanzouRecycleAction,
  lanzouRecycleFiles,
  lanzouRecycleList,
  lanzouRenameFile,
  lanzouRenameFolder,
  lanzouSetFileAccess,
  lanzouSetFileDescription,
  lanzouSetFolderAccess,
  lanzouUploadPrecheck,
} from '@/shared/api'

const filesStore = useFilesStore()
const appStore = useAppStore()
const transferStore = useTransferStore()
const preferenceStore = usePreferenceStore()
const trafficStore = useUploadTrafficStore()
const message = useMessage()
const dialog = useDialog()
const { recycleDeleting, fileDeleting, startRecycleDelete, startFileDelete } = useRecycleDelete()

const selected = ref<LsFile[]>([])
/** Shift 连选锚点：最近一次普通勾选项在排序后全列表中的索引 */
const selectAnchor = ref<number | null>(null)
/** 回收站 Shift 连选锚点 */
const recycleAnchor = ref<number | null>(null)
/** 拖拽/移动中携带的项（含名称与类型，供文件夹模拟移动使用） */
const moveIds = ref<LsFile[]>([])
/** 拖拽悬停的目标文件夹（高亮） */
const dropTargetId = ref('')
/** 文件列表容器（SortableJS 挂载点） */
const listEl = ref<HTMLElement | null>(null)
let sortable: Sortable | null = null
/** 是否正在拖拽 */
const dragActive = ref(false)
/** 拖拽最后位置（用于落点判断） */
const dragPos = ref({ x: 0, y: 0 })
/** 拖拽结束后抑制随后的 click（避免拖拽落点触发进入文件夹/选中） */
const justDragged = ref(false)
/** 移动进行中：冻结页面，防止期间执行其他操作干扰 */
const moving = ref(false)

// 搜索状态
const searchQuery = ref('')
const searchDebounceTimer = ref<ReturnType<typeof setTimeout> | null>(null)

function onSearchInput(value: string) {
  if (searchDebounceTimer.value) clearTimeout(searchDebounceTimer.value)
  searchDebounceTimer.value = setTimeout(() => {
    searchQuery.value = value.trim()
    page.value = 1
    recyclePage.value = 1
  }, 150)
}

function clearSearch() {
  searchQuery.value = ''
  page.value = 1
  recyclePage.value = 1
}

// 回收站状态
const recycleMode = ref(false)
const recycleItems = ref<RecycleItem[]>([])
const recycleLoading = ref(false)
const recycleSelected = ref<RecycleItem[]>([])
const recycleContextItem = ref<RecycleItem | null>(null)
const recycleDropdownShow = ref(false)
const recycleDropdownPos = ref({ x: 0, y: 0 })
/** 正在查看其子文件的回收站文件夹 */
const recycleFolderView = ref<RecycleItem | null>(null)
const recycleFiles = ref<RecycleFile[]>([])
const recycleFilesLoading = ref(false)

// 回收站分页（每页 30 项）
const RECYCLE_PAGE_SIZE = 30
const recyclePage = ref(1)
const filteredRecycleItems = computed(() => {
  const q = searchQuery.value.toLowerCase()
  if (!q) return recycleItems.value
  return recycleItems.value.filter((f) => f.name.toLowerCase().includes(q))
})
const recycleTotalPages = computed(() => Math.max(1, Math.ceil(filteredRecycleItems.value.length / RECYCLE_PAGE_SIZE)))
const recyclePageItems = computed(() => {
  const start = (recyclePage.value - 1) * RECYCLE_PAGE_SIZE
  return filteredRecycleItems.value.slice(start, start + RECYCLE_PAGE_SIZE)
})
watch(recycleTotalPages, (tp) => {
  if (recyclePage.value > tp) recyclePage.value = tp
})
// 列表变化时重置 Shift 连选锚点
watch(
  () => filesStore.files,
  () => {
    selectAnchor.value = null
  },
)
watch(recycleItems, () => {
  recycleAnchor.value = null
})
// 登录态变化：登录后自动加载当前列表，登出后清空数据
watch(
  () => appStore.isLoggedIn,
  async (loggedIn) => {
    if (loggedIn) {
      if (recycleMode.value) await loadRecycle()
      else await loadFiles(filesStore.folderId)
    } else {
      filesStore.files = []
      selected.value = []
      recycleItems.value = []
      recycleSelected.value = []
      recycleFiles.value = []
      recycleFolderView.value = null
    }
  },
)
const dropdownPos = ref({ x: 0, y: 0 })
const dropdownShow = ref(false)
const contextFile = ref<LsFile | null>(null)
const dragging = ref(false)
let unlistenDrop: Promise<() => void> | null = null
/** 上传完成后需删除的旧文件（上传任务 ID → 旧文件） */
const overwriteDeleteMap = new Map<string, LsFile>()

// 上传下拉菜单
const uploadDropdownShow = ref(false)
const uploadDropdownOptions: DropdownOption[] = [
  { label: '选择文件', key: 'file', icon: () => h(NIcon, null, { default: () => h(DocumentOutline) }) },
  { label: '选择文件夹', key: 'folder', icon: () => h(NIcon, null, { default: () => h(FolderOutline) }) },
]

function onUploadSelect(key: string) {
  uploadDropdownShow.value = false
  if (key === 'file') pickFiles()
  else if (key === 'folder') pickFolder()
}

// 新建文件夹
const showMkdir = ref(false)
const newFolderName = ref('')
// 重命名
const showRename = ref(false)
const renameFile = ref<LsFile | null>(null)
const renameName = ref('')
// 设置访问密码
const showAccess = ref(false)
const accessPwd = ref('')
const accessShows = ref(false)

function openAccess() {
  accessPwd.value = ''
  accessShows.value = false
  showAccess.value = true
}

// 蓝奏云要求密码长度 2-6 位，不含空格（服务端实际接受全角符号等字符）
const accessPwdValid = computed(() => /^\S{2,6}$/.test(accessPwd.value))
// 分享链接
const showShareLink = ref(false)
const shareLinks = ref('')
const shareQrCodes = ref<{ url: string; name: string; dataUrl: string }[]>([])
// 移动
const showMove = ref(false)
const moveTreeData = ref<TreeOption[]>([])
const moveSelectedKey = ref<number | null>(null)
const moveLoadingKeys = ref<number[]>([])
const moveExpandedKeys = ref<number[]>([])
// 添加描述
const showDesc = ref(false)
const descFile = ref<LsFile | null>(null)
const descText = ref('')

const currentFolderName = computed(() => filesStore.crumbs[filesStore.crumbs.length - 1]?.name || '根目录')

// 排序状态
const sortKey = ref<'name' | 'size' | 'time' | 'downloads' | null>(null)
const sortAsc = ref(true)

function toggleSort(key: 'name' | 'size' | 'time' | 'downloads') {
  if (sortKey.value === key) {
    // 同一列再次点击：升序 → 降序 → 默认（不排序）
    if (sortAsc.value) {
      sortAsc.value = false
    } else {
      sortKey.value = null
      sortAsc.value = true
    }
  } else {
    sortKey.value = key
    sortAsc.value = true
  }
}

function sortArrowClass(key: 'name' | 'size' | 'time' | 'downloads') {
  return sortKey.value === key ? 'active' : ''
}

// 排序后的文件列表（文件夹始终在顶部，支持搜索过滤）
const sortedFiles = computed(() => {
  let list = [...filesStore.files]
  // 搜索过滤
  const q = searchQuery.value.toLowerCase()
  if (q) {
    list = list.filter((f) => f.name.toLowerCase().includes(q))
  }
  const k = sortKey.value
  if (!k) return list
  const dir = sortAsc.value ? 1 : -1
  return list.sort((a, b) => {
    // 文件夹始终在顶部，文件在后
    if (a.type !== b.type) return a.type === 'folder' ? -1 : 1
    if (k === 'name') {
      return a.name.localeCompare(b.name, 'zh-Hans-CN') * dir
    }
    if (k === 'downloads') {
      const na = Number(a.downs || 0)
      const nb = Number(b.downs || 0)
      return (na - nb) * dir
    }
    if (k === 'size') {
      const na = parseSizeText(a.size || '')
      const nb = parseSizeText(b.size || '')
      return (na - nb) * dir
    }
    if (k === 'time') {
      return (a.time || '').localeCompare(b.time || '') * dir
    }
    return 0
  })
})

// 注意：必须放在 sortedFiles 之后——watch 会立即读取 source，
// totalPages/pageFiles 引用了 sortedFiles，声明在后会触发 TDZ
const PAGE_SIZE = 30
const page = ref(1)
const totalPages = computed(() => Math.max(1, Math.ceil(sortedFiles.value.length / PAGE_SIZE)))
// 当前页数据（先排序后分页）
const pageFiles = computed(() => {
  const start = (page.value - 1) * PAGE_SIZE
  return sortedFiles.value.slice(start, start + PAGE_SIZE)
})

// 切换目录/排序变化时回到第一页；文件数变化时校正页码
watch([() => filesStore.folderId, sortKey, sortAsc], () => {
  page.value = 1
})
watch(totalPages, (tp) => {
  if (page.value > tp) page.value = tp
})

const allSelected = computed(() => {
  const list = filesStore.files
  return list.length > 0 && list.every((f) => selected.value.some((s) => s.id === f.id))
})

function toggleSelectAll() {
  if (allSelected.value) {
    selected.value = []
  } else {
    selected.value = filesStore.files.slice()
  }
}

onMounted(async () => {
  setRecycleDeleteFinish(async () => {
    recycleSelected.value = []
    await loadRecycle()
  })
  setFileDeleteFinish(async () => {
    selected.value = []
    await refresh()
  })
  transferStore.onUploadDone(async (id) => {
    await refresh()
    const oldFile = overwriteDeleteMap.get(id)
    if (oldFile) {
      overwriteDeleteMap.delete(id)
      startFileDelete([oldFile])
    }
  })
  await loadFiles(-1)
  if ('__TAURI_INTERNALS__' in window) {
    window.addEventListener('files:refresh', refresh)
    const { getCurrentWebview } = await import('@tauri-apps/api/webview')
    const webview = getCurrentWebview()
    unlistenDrop = webview.onDragDropEvent((event) => {
      if (event.payload.type === 'enter') {
        if (moveIds.value.length) return
        dragging.value = true
      } else if (event.payload.type === 'leave') {
        dragging.value = false
      } else if (event.payload.type === 'drop') {
        dragging.value = false
        const paths = event.payload.paths
        if (paths?.length) {
          handleDroppedPaths(paths)
        }
      }
    })
  }
  window.addEventListener('mousemove', onWindowMouseMove)
  await nextTick()
  initSortable()
  syncScrollbarGutter()
})

onUnmounted(() => {
  setRecycleDeleteFinish(null)
  setFileDeleteFinish(null)
  window.removeEventListener('files:refresh', refresh)
  unlistenDrop?.then((fn) => fn())
  window.removeEventListener('mousemove', onWindowMouseMove)
  destroySortable()
})

async function handleDroppedPaths(paths: string[]) {
  if (!appStore.isLoggedIn) {
    message.warning('请先登录')
    return
  }
  if (!(await checkUploadWarning())) return
  if (!paths.length) return

  const names = paths.map((p) => p.split(/[\\/]/).pop() || p)
  const nameList = names.join('\n')
  const confirmed = await new Promise<boolean>((resolve) => {
    dialog.info({
      title: '确认上传',
      content: () => h('div', null, [
        h('div', { style: 'margin-bottom: 8px;' }, `确定上传以下 ${paths.length} 个项目吗？`),
        h('div', {
          style: 'max-height: 200px; overflow-y: auto; font-size: 12px; color: var(--m3-on-surface-variant); background: var(--m3-surface-container-highest); padding: 8px; border-radius: 6px; white-space: pre-wrap;',
        }, nameList),
      ]),
      positiveText: '上传',
      negativeText: '取消',
      transformOrigin: 'center',
      onPositiveClick: () => resolve(true),
      onNegativeClick: () => resolve(false),
      onClose: () => resolve(false),
    })
  })
  if (!confirmed) return

  for (const path of paths) {
    await startUploadWithPrecheck(path)
  }
}

// 拖拽排序
function initSortable() {
  if (sortable || !listEl.value) return
  sortable = Sortable.create(listEl.value, {
    group: 'files',
    // 不做列表重排，仅提供拖拽手势；落点由鼠标位置判断
    sort: false,
    forceFallback: true,
    fallbackOnBody: true,
    fallbackTolerance: 3,
    draggable: '.file-row:not(.file-head)',
    ghostClass: 'file-sortable-ghost',
    chosenClass: 'file-sortable-chosen',
    fallbackClass: 'file-sortable-fallback',
    filter: '.col-check, button, a, input, textarea, select',
    preventOnFilter: false,
    onStart: (evt: SortableEvent) => {
      const id = (evt.item as HTMLElement).dataset.id
      const file = filesStore.files.find((f) => f.id === id)
      if (file) moveIds.value = selected.value.length ? [...selected.value] : [file]
      dragActive.value = true
    },
    onEnd: () => {
      dragActive.value = false
      justDragged.value = true
      // 若本次 click 未落在行上（拖到空白处），下一 tick 复位，避免吞掉后续点击
      setTimeout(() => {
        justDragged.value = false
      }, 0)
      dropTargetId.value = ''
      handleDropTarget()
    },
  })
}

function destroySortable() {
  if (sortable) {
    sortable.destroy()
    sortable = null
  }
}

/** 拖拽过程中跟随鼠标：更新落点高亮 */
function onWindowMouseMove(e: MouseEvent) {
  if (!dragActive.value) return
  dragPos.value = { x: e.clientX, y: e.clientY }
  const el = document.elementFromPoint(e.clientX, e.clientY) as HTMLElement | null
  const folderRow = el?.closest<HTMLElement>('.file-row.folder')
  dropTargetId.value = folderRow ? (folderRow.dataset.id ?? '') : ''
}

/** 松手：判断鼠标落在哪个文件夹行上，执行移动 */
function handleDropTarget() {
  const el = document.elementFromPoint(dragPos.value.x, dragPos.value.y) as HTMLElement | null
  const folderRow = el?.closest<HTMLElement>('.file-row.folder')
  const target = folderRow ? filesStore.files.find((f) => f.id === folderRow.dataset.id) : undefined
  if (target && target.type === 'folder' && !moveIds.value.some((f) => f.id === target.id)) {
    void onDropFolder(target)
  } else {
    moveIds.value = []
  }
}

// 列表在空/非空间切换时容器会重建，需重新挂载 Sortable
watch(
  () => filesStore.files.length,
  async (len) => {
    await nextTick()
    if (len === 0) destroySortable()
    else initSortable()
    syncScrollbarGutter()
  },
)

// 进入回收站视图时文件表格卸载，销毁 Sortable；返回时重新挂载
watch(recycleMode, async (v) => {
  await nextTick()
  if (v) destroySortable()
  else initSortable()
})

/** 实测滚动条/预留槽宽度，用于表头与数据行右对齐（消除滚动条挤压与抖动） */
function syncScrollbarGutter() {
  const el = listEl.value
  if (!el) return
  const sbw = el.offsetWidth - el.clientWidth
  if (sbw >= 0) document.documentElement.style.setProperty('--scrollbar-w', `${sbw}px`)
}

async function loadFiles(fid: number) {
  if (!appStore.isLoggedIn) {
    filesStore.files = []
    selected.value = []
    return
  }
  await filesStore.load(fid)
  selected.value = []
}

function refresh() {
  return loadFiles(filesStore.folderId)
}

async function enterFolder(file: LsFile) {
  if (file.type !== 'folder') return
  await loadFiles(Number(file.id))
}

function goCrumbs(idx: number) {
  const target = filesStore.crumbs[idx]
  if (target) loadFiles(Number(target.id))
}

function toggleSelect(file: LsFile) {
  const idx = selected.value.findIndex((s) => s.id === file.id)
  if (idx >= 0) {
    selected.value.splice(idx, 1)
  } else {
    selected.value.push(file)
  }
}

function isSelected(file: LsFile) {
  return selected.value.some((s) => s.id === file.id)
}

/** 复选框点击：Shift 时按锚点区间连选；普通点击更新锚点 */
function onCheckClick(file: LsFile, event: MouseEvent) {
  if (event.shiftKey && selectAnchor.value != null) {
    const cur = sortedFiles.value.findIndex((f) => f.id === file.id)
    if (cur >= 0) {
      selected.value = expandRangeSelection(selected.value, sortedFiles.value, selectAnchor.value, cur)
      return
    }
  }
  toggleSelect(file)
  selectAnchor.value = sortedFiles.value.findIndex((f) => f.id === file.id)
}

// 回收站操作
async function toggleRecycle() {
  recycleMode.value = !recycleMode.value
  recycleSelected.value = []
  if (recycleMode.value) {
    // 后台删除进行中：不重复请求，由删除完成的 onFinish 回调刷新列表
    if (!recycleDeleting.value) await loadRecycle()
  }
}

async function loadRecycle() {
  if (!appStore.isLoggedIn) {
    recycleItems.value = []
    recycleSelected.value = []
    recyclePage.value = 1
    return
  }
  recycleLoading.value = true
  try {
    recycleItems.value = await lanzouRecycleList()
    recycleSelected.value = []
    recyclePage.value = 1
  } catch (e) {
    message.error(e instanceof Error ? e.message : String(e))
  } finally {
    recycleLoading.value = false
  }
}

function toggleRecycleSelect(item: RecycleItem) {
  const idx = recycleSelected.value.findIndex((s) => s.id === item.id)
  if (idx >= 0) recycleSelected.value.splice(idx, 1)
  else recycleSelected.value.push(item)
}

function isRecycleSelected(item: RecycleItem) {
  return recycleSelected.value.some((s) => s.id === item.id)
}

/** 回收站复选框点击：Shift 时按锚点区间连选；普通点击更新锚点 */
function onRecycleCheckClick(item: RecycleItem, event: MouseEvent) {
  if (event.shiftKey && recycleAnchor.value != null) {
    const cur = recycleItems.value.findIndex((i) => i.id === item.id)
    if (cur >= 0) {
      recycleSelected.value = expandRangeSelection(recycleSelected.value, recycleItems.value, recycleAnchor.value, cur)
      return
    }
  }
  toggleRecycleSelect(item)
  recycleAnchor.value = recycleItems.value.findIndex((i) => i.id === item.id)
}

const recycleAllSelected = computed(
  () => recycleItems.value.length > 0 && recycleSelected.value.length === recycleItems.value.length,
)

function toggleRecycleSelectAll() {
  if (recycleAllSelected.value) recycleSelected.value = []
  else recycleSelected.value = recycleItems.value.slice()
}

async function restoreSelected() {
  const list = recycleSelected.value.length
    ? recycleSelected.value
    : recycleContextItem.value
      ? [recycleContextItem.value]
      : []
  if (!list.length) return
  moving.value = true
  try {
    for (const it of list) {
      await lanzouRecycleAction(it.id, it.type, 'restore')
    }
    message.success(`已恢复 ${list.length} 项`)
    recycleSelected.value = []
    await loadRecycle()
  } catch (e) {
    message.error(e instanceof Error ? e.message : String(e))
  } finally {
    moving.value = false
  }
}

function deleteRecycleSelected(items: RecycleItem[]) {
  if (!items.length || recycleDeleting.value) return
  dialog.error({
    title: '彻底删除',
    content: `确定彻底删除选中的 ${items.length} 项吗？删除后不可恢复！`,
    positiveText: '彻底删除',
    negativeText: '取消',
    onPositiveClick: () => {
      startRecycleDelete(items)
    },
  })
}

function onRecycleContext(item: RecycleItem, event: MouseEvent) {
  recycleContextItem.value = item
  if (!isRecycleSelected(item)) recycleSelected.value = [item]
  recycleDropdownPos.value = { x: event.clientX, y: event.clientY }
  recycleDropdownShow.value = true
}

const recycleContextOptions: DropdownOption[] = [
  { label: '恢复', key: 'restore', icon: () => h(NIcon, null, { default: () => h(RefreshOutline) }) },
  {
    label: '彻底删除',
    key: 'delete',
    icon: () => h(NIcon, null, { default: () => h(TrashOutline) }),
    props: { class: 'dropdown-option-danger' },
  },
]

async function onRecycleContextSelect(key: string) {
  recycleDropdownShow.value = false
  if (!recycleContextItem.value) return
  if (key === 'restore') await restoreSelected()
  else if (key === 'delete') deleteRecycleSelected([recycleContextItem.value])
}

// 回收站子文件夹
async function enterRecycleFolder(item: RecycleItem) {
  if (item.type !== 'folder') return
  recycleFolderView.value = item
  recycleSelected.value = []
  await loadRecycleFiles(item.id)
}

function backToRecycleRoot() {
  recycleFolderView.value = null
  recycleFiles.value = []
}

async function loadRecycleFiles(folderId: string) {
  recycleFilesLoading.value = true
  try {
    recycleFiles.value = await lanzouRecycleFiles(folderId)
  } catch (e) {
    message.error(e instanceof Error ? e.message : String(e))
  } finally {
    recycleFilesLoading.value = false
  }
}

/** 回收站行点击：文件夹进入查看子文件；文件行点击不改变选中（选中仅通过复选框） */
function onRecycleRowClick(item: RecycleItem) {
  if (item.type === 'folder') void enterRecycleFolder(item)
}

function onRowClick(file: LsFile, event: MouseEvent) {
  if (event.button === 2) {
    contextFile.value = file
    if (!isSelected(file)) {
      selected.value = [file]
    }
    dropdownPos.value = { x: event.clientX, y: event.clientY }
    dropdownShow.value = true
    return
  }
  // 刚结束拖拽：抑制本次 click，避免误进入文件夹/选中
  if (justDragged.value) {
    justDragged.value = false
    return
  }
  // 左键点击：关闭右键菜单
  dropdownShow.value = false
  // 文件夹左键进入；文件行点击不改变选中（选中仅通过复选框）
  if (file.type === 'folder') {
    enterFolder(file)
  }
}

const contextOptions: DropdownOption[] = [
  { label: '下载', key: 'download', icon: () => h(NIcon, null, { default: () => h(CloudDownloadOutline) }) },
  { label: '移动', key: 'move', icon: () => h(NIcon, null, { default: () => h(MoveOutline) }) },
  { label: '重命名', key: 'rename', icon: () => h(NIcon, null, { default: () => h(CreateOutline) }) },
  { label: '分享', key: 'share', icon: () => h(NIcon, null, { default: () => h(LinkOutline) }) },
  { label: '设置密码', key: 'access', icon: () => h(NIcon, null, { default: () => h(KeyOutline) }) },
  { label: '添加描述', key: 'desc', icon: () => h(NIcon, null, { default: () => h(DocumentTextOutline) }) },
  {
    label: '删除',
    key: 'delete',
    icon: () => h(NIcon, null, { default: () => h(TrashOutline) }),
    props: { class: 'dropdown-option-danger' },
  },
]

async function onContextSelect(key: string) {
  const file = contextFile.value
  if (!file) return
  dropdownShow.value = false
  switch (key) {
    case 'download':
      await doDownload([file])
      break
    case 'share':
      await doShare()
      break
    case 'access':
      openAccess()
      break
    case 'rename':
      renameFile.value = file
      renameName.value = file.name
      showRename.value = true
      break
    case 'move':
      await openMoveDialog([file])
      break
    case 'desc':
      await openDesc(file)
      break
    case 'delete':
      await doDelete()
      break
  }
}

async function pickFiles() {
  const picked = await openDialog({
    multiple: true,
    directory: false,
    filters: [{ name: '所有文件', extensions: ['*'] }],
  })
  if (picked) {
    if (!(await checkUploadWarning())) return
    const list = Array.isArray(picked) ? picked : [picked]
    for (const p of list) {
      await startUploadWithPrecheck(p)
    }
  }
}

async function pickFolder() {
  const picked = await openDialog({ directory: true })
  const p = Array.isArray(picked) ? picked[0] : picked
  if (p) {
    if (!(await checkUploadWarning())) return
    await startUploadWithPrecheck(p)
  }
}

async function startUpload(path: string, chunkOversized?: boolean, overwriteFile?: LsFile) {
  const id = `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`
  const name = path.split(/[\\/]/).pop() || path
  if (overwriteFile) overwriteDeleteMap.set(id, overwriteFile)
  const item = {
    id,
    name,
    kind: 'upload' as const,
    status: 'pending' as const,
    uploaded: 0,
    total: 0,
    speed: 0,
    payload: path,
    folderId: filesStore.folderId,
    chunkOversized,
  }
  transferStore.addUpload(item)
  transferStore.startAll('upload')
  message.success(`开始上传: ${name}`)
}

/** 上传流量警戒：今日累计流量超过警戒线时询问是否继续（返回 false 则中止添加上传；未设置警戒线视为关闭） */
async function checkUploadWarning(): Promise<boolean> {
  const warningSize = preferenceStore.config.uploadWarningSize
  if (warningSize == null) return true
  const size = trafficStore.todaySize()
  const limit = warningSize * 1024 ** 3
  if (size < limit) return true
  return new Promise<boolean>((resolve) => {
    dialog.warning({
      title: '上传流量提醒',
      content: `当天上传总流量（${formatSize(size)}）已超过警戒线（${warningSize} GB），是否继续上传？`,
      positiveText: '继续上传',
      negativeText: '取消',
      transformOrigin: 'center',
      onPositiveClick: () => resolve(true),
      onNegativeClick: () => resolve(false),
      onClose: () => resolve(false),
    })
  })
}

/**
 * 上传预检：校验文件类型 + 所选路径存在超出账号单文件限制的文件时，按用户选择决定是否分片上传。
 * 单文件：询问「是否分片上传？」（否=取消上传）；文件夹：询问「是否将超限文件全部分片上传？」（跳过=跳过超限文件）
 */
async function startUploadWithPrecheck(path: string) {
  const fileName = path.split(/[\\/]/).pop() || path
  const duplicate = filesStore.files.find((f) => f.name === fileName)

  // 文件重名：弹窗确认
  if (duplicate && duplicate.type === 'file') {
    const confirmed = await new Promise<boolean>((resolve) => {
      dialog.warning({
        title: '文件已存在',
        content: `当前目录已存在同名文件「${fileName}」，是否覆盖？`,
        positiveText: '覆盖',
        negativeText: '跳过',
        transformOrigin: 'center',
        onPositiveClick: () => resolve(true),
        onNegativeClick: () => resolve(false),
        onClose: () => resolve(false),
      })
    })
    if (!confirmed) return
    await startUpload(path, false, duplicate)
    return
  }

  // 文件类型校验（仅文件，文件夹由服务端逐文件校验）
  const isFile = /\.[^\\/]+$/.test(path)
  if (isFile) {
    const supportList = appStore.profile?.supportList
    if (supportList?.length) {
      const ext = path.split(/[\\/]/).pop()?.split('.').pop()?.toLowerCase()
      if (ext && !supportList.some((s) => s.toLowerCase() === ext)) {
        message.warning(`不支持的文件类型: .${ext}`)
        return
      }
    }
  }
  let res
  try {
    res = await lanzouUploadPrecheck(path)
  } catch (e) {
    message.error(e instanceof Error ? e.message : String(e))
    return
  }
  const oversized = res.oversized
  if (!oversized.length) {
    await startUpload(path)
    return
  }
  const limitText = res.maxSize ? formatSize(res.maxSize) : ''
  const isSingle = oversized.length === 1 && oversized[0].path === path
  if (isSingle) {
    const split = await new Promise<boolean>((resolve) => {
      dialog.warning({
        title: '文件大小超出限制',
        content: `文件大小超出账号单个文件限制${limitText ? `（${limitText}）` : ''}，是否分片上传？`,
        positiveText: '是',
        negativeText: '否',
        transformOrigin: 'center',
        onPositiveClick: () => resolve(true),
        onNegativeClick: () => resolve(false),
        onClose: () => resolve(false),
      })
    })
    if (split) await startUpload(path, true)
  } else {
    const chunkAll = await new Promise<boolean>((resolve) => {
      dialog.warning({
        title: '存在超限文件',
        content: `存在 ${oversized.length} 个文件大小超出限制${limitText ? `（${limitText}）` : ''}，是否将超出大小限制文件全部分片上传？`,
        positiveText: '确定',
        negativeText: '跳过',
        transformOrigin: 'center',
        onPositiveClick: () => resolve(true),
        onNegativeClick: () => resolve(false),
        onClose: () => resolve(false),
      })
    })
    await startUpload(path, chunkAll)
  }
}

async function doMkdir() {
  if (!newFolderName.value.trim()) return
  moving.value = true
  try {
    await lanzouMkdir(filesStore.folderId, newFolderName.value.trim())
    message.success('文件夹创建成功')
    showMkdir.value = false
    newFolderName.value = ''
    await refresh()
  } catch (e) {
    message.error(e instanceof Error ? e.message : String(e))
  } finally {
    moving.value = false
  }
}

async function doRename() {
  const file = renameFile.value
  if (!file || !renameName.value.trim()) return
  moving.value = true
  try {
    if (file.type === 'folder') {
      await lanzouRenameFolder(file.id, renameName.value.trim())
    } else {
      await lanzouRenameFile(file.id, renameName.value.trim())
    }
    message.success('重命名成功')
    showRename.value = false
    await refresh()
  } catch (e) {
    message.error(e instanceof Error ? e.message : String(e))
  } finally {
    moving.value = false
  }
}

function doDelete() {
  const files = selected.value.length ? selected.value : contextFile.value ? [contextFile.value] : []
  if (!files.length || fileDeleting.value) return
  dialog.error({
    title: '删除',
    content: `确定删除选中的 ${files.length} 项吗？`,
    positiveText: '删除',
    negativeText: '取消',
    transformOrigin: 'center',
    onPositiveClick: () => {
      startFileDelete(files)
    },
  })
}

async function doShare() {
  // 一键分享：未选中时分享当前页全部文件
  const files = selected.value.length ? selected.value : contextFile.value ? [contextFile.value] : filesStore.files
  if (!files.length) return
  const lines: string[] = []
  const qrItems: { url: string; name: string }[] = []
  moving.value = true
  try {
    for (const f of files) {
      const detail = f.type === 'folder' ? await lanzouFolderDetail(f.id) : await lanzouFileDetail(f.id)
      const link = `${detail.url || ''}${detail.pwd ? ` 密码:${detail.pwd}` : ''}`
      lines.push(`${f.name} ${link}`)
      if (detail.url) {
        qrItems.push({ url: detail.url, name: f.name })
      }
    }
    shareLinks.value = lines.join('\n')
    // 生成二维码
    const qrCodes = await Promise.all(
      qrItems.map((item) =>
        QRCode.toDataURL(item.url, {
          width: 190,
          margin: 2,
          color: { dark: '#3f3f3f', light: '#ffffff' },
          errorCorrectionLevel: 'H',
        }).then((dataUrl) => ({ url: item.url, name: item.name, dataUrl })),
      ),
    )
    shareQrCodes.value = qrCodes
    showShareLink.value = true
  } catch (e) {
    message.error(e instanceof Error ? e.message : String(e))
  } finally {
    moving.value = false
  }
}

async function doSetAccess() {
  const files = selected.value.length ? selected.value : contextFile.value ? [contextFile.value] : []
  if (!files.length) return
  if (accessShows.value && !accessPwdValid.value) {
    message.warning('密码需为 2-6 位且不能包含空格')
    return
  }
  moving.value = true
  try {
    const shows = accessShows.value ? 1 : 0
    for (const f of files) {
      if (f.type === 'folder') {
        await lanzouSetFolderAccess(f.id, shows, accessPwd.value)
      } else {
        await lanzouSetFileAccess(f.id, shows, accessPwd.value)
      }
    }
    message.success('设置成功')
    showAccess.value = false
    selected.value = []
    await refresh()
  } catch (e) {
    message.error(e instanceof Error ? e.message : String(e))
  } finally {
    moving.value = false
  }
}

/** 递归收集文件夹内的文件，映射到本地子目录（relPath 为相对下载目录的路径） */
async function collectFolderFiles(folder: LsFile, relPath: string, out: Array<{ file: LsFile; dir?: string }>) {
  const res = await lanzouLs(Number(folder.id), true)
  for (const f of res.files) {
    if (f.type === 'folder') {
      await collectFolderFiles(f, relPath ? `${relPath}/${f.name}` : f.name, out)
    } else {
      out.push({ file: f, dir: relPath })
    }
  }
}

async function doDownload(files: LsFile[]) {
  const list = files.length
    ? files
    : selected.value.length
      ? selected.value
      : contextFile.value
        ? [contextFile.value]
        : []
  if (!list.length) return
  moving.value = true
  try {
    // 文件夹展开为子任务：文件直接下载；文件夹递归展开，目录结构映射到本地
    const tasks: Array<{ file: LsFile; dir?: string }> = []
    for (const f of list) {
      if (f.type === 'folder') {
        await collectFolderFiles(f, f.name, tasks)
      } else {
        tasks.push({ file: f })
      }
    }
    let added = 0
    for (const { file, dir } of tasks) {
      const id = `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`
      transferStore.addDownload({
        id,
        name: file.name,
        kind: 'download',
        status: 'pending',
        uploaded: 0,
        total: 0,
        speed: 0,
        payload: file.id,
        dir,
      })
      added++
    }
    if (added) {
      message.success(`已添加 ${added} 项到下载列表`)
      transferStore.startAll('download')
    }
  } finally {
    moving.value = false
  }
}

/** 当前多选是否构成完整分片组（全为分片名 + 序号连续从 1 开始），可用于合并下载 */
const mergeGroup = computed(() => {
  const files = selected.value.filter((f) => f.type === 'file')
  if (files.length < 2) return null
  if (!files.every((f) => isPartName(f.name))) return null
  const merged = commonMergedName(files.map((f) => f.name))
  if (!merged) return null
  const idxs = files.map((f) => parsePartIndex(f.name)).sort((a, b) => a - b)
  if (idxs.length !== idxs[idxs.length - 1] || !idxs.every((v, i) => v === i + 1)) return null
  return { merged, files }
})

async function doMergeDownload() {
  const group = mergeGroup.value
  if (!group) return
  const keep = await new Promise<boolean>((resolve) => {
    dialog.warning({
      title: '合并下载',
      content: `将下载 ${group.files.length} 个分片文件并合并为「${group.merged}」，是否保留分片文件？`,
      positiveText: '保留',
      negativeText: '删除',
      transformOrigin: 'center',
      onPositiveClick: () => resolve(true),
      onNegativeClick: () => resolve(false),
      onClose: () => resolve(false),
    })
  })
  const id = `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`
  transferStore.addDownload({
    id,
    name: group.merged,
    kind: 'download',
    status: 'pending',
    uploaded: 0,
    total: 0,
    speed: 0,
    mergeFiles: group.files.map((f) => ({ id: f.id, name: f.name })),
    keepParts: keep,
  })
  transferStore.startAll('download')
  message.success(`开始合并下载: ${group.merged}`)
}

async function openMoveDialog(files: LsFile[]) {
  moveIds.value = files
  try {
    // 树根目录：根目录(-1)
    moveTreeData.value = [{ key: -1, label: '根目录', isLeaf: false, children: [] }]
    const children = await loadMoveChildren(-1)
    moveTreeData.value[0].children = children
    moveTreeData.value[0].isLeaf = children.length === 0

    // 自动展开到当前文件夹
    const crumbs = filesStore.crumbs
    const expanded: number[] = [-1]
    let currentNode = moveTreeData.value[0]
    // 从 index 1 开始（跳过根目录），逐层加载并展开
    for (let i = 1; i < crumbs.length; i++) {
      const id = Number(crumbs[i].id)
      expanded.push(id)
      // 如果子节点还没加载，先加载
      if (!currentNode.children?.length) {
        const kids = await loadMoveChildren(id)
        currentNode.children = kids
        currentNode.isLeaf = kids.length === 0
      }
      const found = currentNode.children?.find((c) => c.key === id)
      if (found) {
        currentNode = found
      } else {
        break
      }
    }
    moveExpandedKeys.value = expanded
    // 选中当前文件夹
    moveSelectedKey.value = Number(crumbs[crumbs.length - 1].id)
    showMove.value = true
  } catch (e) {
    message.error(e instanceof Error ? e.message : String(e))
  }
}

const canMove = computed(() => moveSelectedKey.value !== null)

function onMoveSelect(keys: Array<number | string>) {
  moveSelectedKey.value = keys.length ? (keys[0] as number) : null
}

async function doMove() {
  if (moveSelectedKey.value === null) return
  const targetId = moveSelectedKey.value
  if (moveIds.value.some((f) => f.id === String(targetId))) {
    message.error('不能移动到自身或其子文件夹')
    return
  }
  try {
    await moveToTarget(moveIds.value, targetId)
    message.success('移动成功')
    showMove.value = false
    selected.value = []
    await refresh()
  } catch (e) {
    message.error(e instanceof Error ? e.message : String(e))
  }
}

/** 加载某文件夹的子文件夹（懒加载） */
async function loadMoveChildren(fid: number): Promise<TreeOption[]> {
  try {
    const result = await lanzouLs(fid, true)
    return result.files
      .filter((f) => f.type === 'folder')
      .map((f) => ({
        key: Number(f.id),
        label: f.name,
        isLeaf: false,
        // 注意：不能带 children: []——treemate 视"isLeaf=false 且有 children 数组"为已加载，
        // 懒加载将不再触发，深层目录无法展开
      }))
  } catch (e) {
    message.error(e instanceof Error ? e.message : String(e))
    return []
  }
}

/** NTree on-load：异步加载子节点 */
async function onMoveLoad(node: TreeOption) {
  const key = node.key as number
  moveLoadingKeys.value = [...moveLoadingKeys.value, key]
  try {
    const children = await loadMoveChildren(key)
    node.children = children
    node.isLeaf = children.length === 0
  } finally {
    moveLoadingKeys.value = moveLoadingKeys.value.filter((k) => k !== key)
  }
}

/** 判断目标文件夹是否为被移动项自身或其子孙（根据当前路径祖先链） */
function isAncestorOfTarget(file: LsFile) {
  if (moveIds.value.some((f) => f.id === file.id)) return true
  // 目标文件夹的祖先 = 面包屑路径 + 当前文件夹
  const ancestors = [...filesStore.crumbs.map((c) => c.id), String(filesStore.folderId)]
  return moveIds.value.some((f) => ancestors.includes(f.id))
}

/** 批量移动（文件直移；文件夹模拟移动） */
async function moveToTarget(items: LsFile[], targetId: number) {
  moving.value = true
  try {
    await lanzouMove(
      items.map((f) => ({ id: f.id, name: f.name, type: f.type })),
      targetId,
    )
  } finally {
    moving.value = false
  }
}

async function onDropFolder(file: LsFile) {
  if (file.type !== 'folder' || !moveIds.value.length) return
  if (isAncestorOfTarget(file)) {
    message.error('不能移动到自身或其子文件夹')
    moveIds.value = []
    return
  }
  try {
    await moveToTarget(moveIds.value, Number(file.id))
    message.success('移动成功')
    moveIds.value = []
    selected.value = []
    await refresh()
  } catch (e) {
    message.error(e instanceof Error ? e.message : String(e))
    moveIds.value = []
  }
}

function copyShare() {
  if ('__TAURI_INTERNALS__' in window) {
    navigator.clipboard.writeText(shareLinks.value)
    message.success('已复制')
  }
}

async function openDesc(file: LsFile) {
  descFile.value = file
  descText.value = ''
  try {
    if (file.type === 'file') {
      const info = await lanzouFileDescription(file.id)
      descText.value = info.desc || ''
    }
  } catch {
    /* ignore */
  }
  showDesc.value = true
}

async function doDesc() {
  const file = descFile.value
  if (!file) return
  moving.value = true
  try {
    if (file.type === 'file') {
      await lanzouSetFileDescription(file.id, descText.value)
    }
    message.success('描述已保存')
    showDesc.value = false
    await refresh()
  } catch (e) {
    message.error(e instanceof Error ? e.message : String(e))
  } finally {
    moving.value = false
  }
}
</script>

<template>
  <div class="files-view" @dragenter.prevent @dragover.prevent @drop.prevent>
    <ViewHeader title="我的文件">
      <NButton size="small" :type="recycleMode ? 'default' : 'primary'" @click="toggleRecycle">
        <template #icon>
          <NIcon>
            <TrashOutline />
          </NIcon>
        </template>
        {{ recycleMode ? '返回' : '回收站' }}
      </NButton>
    </ViewHeader>
    <!-- 工具栏 -->
    <div class="files-toolbar">
      <div class="toolbar-row toolbar-row-top">
        <div class="toolbar-search">
          <NInput
            v-model:value="searchQuery"
            @update:value="onSearchInput"
            placeholder="搜索文件名..."
            size="small"
            clearable
            @clear="clearSearch"
          >
            <template #prefix>
              <NIcon :size="16">
                <SearchOutline />
              </NIcon>
            </template>
          </NInput>
        </div>
        <div class="toolbar-actions">
          <template v-if="recycleMode && !recycleFolderView">
            <NButton size="small" @click="restoreSelected" :disabled="!appStore.isLoggedIn || !recycleSelected.length || recycleDeleting">
              <template #icon>
                <NIcon>
                  <RefreshOutline />
                </NIcon>
              </template>
              恢复 ({{ recycleSelected.length }})
            </NButton>
            <NButton
              size="small"
              type="error"
              :disabled="!appStore.isLoggedIn || !recycleSelected.length || recycleDeleting"
              @click="deleteRecycleSelected(recycleSelected)"
            >
              <template #icon>
                <NIcon>
                  <TrashOutline />
                </NIcon>
              </template>
              彻底删除 ({{ recycleSelected.length }})
            </NButton>
            <NButton size="small" :disabled="!appStore.isLoggedIn || recycleDeleting" @click="loadRecycle">
              <template #icon>
                <NIcon>
                  <RefreshOutline />
                </NIcon>
              </template>
            </NButton>
          </template>
          <template v-else-if="recycleMode && recycleFolderView">
            <NButton size="small" :disabled="!appStore.isLoggedIn" @click="backToRecycleRoot">
              <template #icon>
                <NIcon>
                  <RefreshOutline />
                </NIcon>
              </template>
            </NButton>
          </template>
          <template v-else-if="!selected.length">
            <NDropdown
              v-model:show="uploadDropdownShow"
              :options="uploadDropdownOptions"
              :disabled="!appStore.isLoggedIn"
              trigger="click"
              @select="onUploadSelect"
            >
              <NButton size="small" type="primary" :disabled="!appStore.isLoggedIn">
                <template #icon>
                  <NIcon>
                    <CloudUploadOutline />
                  </NIcon>
                </template>
                上传
              </NButton>
            </NDropdown>
            <NButton size="small" :disabled="!appStore.isLoggedIn" @click="showMkdir = true">
              <template #icon>
                <NIcon>
                  <AddOutline />
                </NIcon>
              </template>
              新建文件夹
            </NButton>
          </template>
          <template v-else>
            <NButton size="small" :disabled="!appStore.isLoggedIn" @click="doShare">
              <template #icon>
                <NIcon>
                  <LinkOutline />
                </NIcon>
              </template>
              分享 ({{ selected.length }})
            </NButton>
            <NButton size="small" :disabled="!appStore.isLoggedIn" @click="openAccess">
              <template #icon>
                <NIcon>
                  <KeyOutline />
                </NIcon>
              </template>
              设置密码
            </NButton>
            <NButton size="small" :disabled="!appStore.isLoggedIn" @click="doDownload(selected)">
              <template #icon>
                <NIcon>
                  <CloudDownloadOutline />
                </NIcon>
              </template>
              下载 ({{ selected.length }})
            </NButton>
            <NButton v-if="mergeGroup" size="small" :disabled="!appStore.isLoggedIn" @click="doMergeDownload">
              <template #icon>
                <NIcon>
                  <DocumentsOutline />
                </NIcon>
              </template>
              合并下载
            </NButton>
            <NButton size="small" :disabled="!appStore.isLoggedIn" @click="openMoveDialog(selected)">
              <template #icon>
                <NIcon>
                  <MoveOutline />
                </NIcon>
              </template>
              移动 ({{ selected.length }})
            </NButton>
            <NButton size="small" type="error" :disabled="!appStore.isLoggedIn || fileDeleting" @click="doDelete">
              <template #icon>
                <NIcon>
                  <TrashOutline />
                </NIcon>
              </template>
              删除 ({{ selected.length }})
            </NButton>
          </template>
          <NButton v-if="!recycleMode" size="small" :disabled="!appStore.isLoggedIn" @click="refresh">
            <template #icon>
              <NIcon>
                <RefreshOutline />
              </NIcon>
            </template>
          </NButton>
        </div>
      </div>
      <div class="toolbar-row toolbar-row-bottom">
        <NBreadcrumb class="breadcrumb">
          <template v-if="recycleMode">
            <NBreadcrumbItem @click="backToRecycleRoot">回收站</NBreadcrumbItem>
            <NBreadcrumbItem v-if="recycleFolderView" :clickable="false">
              {{ recycleFolderView.name }}
            </NBreadcrumbItem>
          </template>
          <template v-else>
            <template v-for="(c, i) in filesStore.crumbs" :key="c.id">
              <NBreadcrumbItem
                v-if="i < filesStore.crumbs.length - 1"
                @click="goCrumbs(i)"
              >
                {{ c.name }}
              </NBreadcrumbItem>
              <NBreadcrumbItem v-else :clickable="false">
                {{ c.name }}
              </NBreadcrumbItem>
            </template>
          </template>
        </NBreadcrumb>
      </div>
    </div>

    <div class="files-body" :class="{ moving }">
      <NSpin :show="recycleMode ? recycleLoading || recycleDeleting || moving : filesStore.loading || moving || fileDeleting">
        <!-- 回收站 -->
        <template v-if="recycleMode">
          <!-- 文件夹内子文件（只读：文件名 + 大小） -->
          <template v-if="recycleFolderView">
            <div class="file-table">
              <div class="file-row file-head">
                <span class="col-name">文件名</span>
                <span class="col-size">大小</span>
              </div>
              <div class="files-scroll">
                <div v-if="!recycleFilesLoading && recycleFiles.length === 0" class="files-empty">
                  <NEmpty description="此文件夹没有包含文件" />
                </div>
                <div v-for="f in recycleFiles" :key="f.name" class="file-row">
                  <span class="col-name">
                    <NIcon class="file-icon" :size="18" :color="getFileIconColor(f.name)">
                      <component :is="getFileIconComponent(f.name)" />
                    </NIcon>
                    <span class="file-name">{{ f.name }}</span>
                  </span>
                  <span class="col-size">{{ f.size || '—' }}</span>
                </div>
              </div>
            </div>
          </template>
          <!-- 回收站根列表 -->
          <template v-else>
            <div v-if="!recycleLoading && !recycleDeleting && recycleItems.length === 0" class="files-empty">
              <NEmpty :description="appStore.isLoggedIn ? '回收站为空' : '请先登录'" />
            </div>
            <div v-else class="file-table">
              <div class="file-row file-head">
                <span class="col-check" @click.stop="toggleRecycleSelectAll">
                  <NCheckbox :checked="recycleAllSelected" />
                </span>
                <span class="col-name">
                  <template v-if="recycleSelected.length > 0">
                    <span class="selection-info">已选择{{ recycleSelected.length }}项 </span>
                    <span class="selection-deselect" @click.stop="recycleSelected = []">取消选择</span>
                  </template>
                  <template v-else>
                    文件名
                    <span class="file-count">(共{{ filteredRecycleItems.length }}项)</span>
                  </template>
                </span>
                <span class="col-size">大小</span>
                <span class="col-time">时间</span>
              </div>
              <div class="files-scroll">
                <div
                  v-for="item in recyclePageItems"
                  :key="`re-${item.type}-${item.id}`"
                  class="file-row"
                  :class="{ selected: isRecycleSelected(item) }"
                  @click="onRecycleRowClick(item)"
                  @contextmenu.prevent="onRecycleContext(item, $event)"
                >
                  <span class="col-check" @click.stop="onRecycleCheckClick(item, $event)">
                    <NCheckbox :checked="isRecycleSelected(item)" />
                  </span>
                  <span class="col-name">
                    <NIcon
                      class="file-icon"
                      :size="18"
                      :color="item.type === 'folder' ? 'var(--m3-primary)' : getFileIconColor(item.name)"
                    >
                      <component :is="item.type === 'folder' ? FolderOutline : getFileIconComponent(item.name)" />
                    </NIcon>
                    <span class="file-name">{{ item.name }}</span>
                  </span>
                  <span class="col-size">{{ item.size || '—' }}</span>
                  <span class="col-time">{{ item.time || '—' }}</span>
                </div>
              </div>
              <div v-if="recycleTotalPages > 1" class="files-pager">
                <NPagination v-model:page="recyclePage" :page-count="recycleTotalPages" />
              </div>
            </div>
          </template>
        </template>
        <!-- 文件列表 -->
        <template v-else>
          <div v-if="!filesStore.loading && filesStore.files.length === 0" class="files-empty">
            <NEmpty :description="appStore.isLoggedIn ? '暂无文件' : '请先登录'" />
          </div>
          <div v-else class="file-table">
            <div class="file-row file-head">
              <span class="col-check" @click.stop="toggleSelectAll">
                <NCheckbox :checked="allSelected" />
              </span>
              <span class="col-name sortable" @click="toggleSort('name')">
                <template v-if="selected.length > 0">
                  <span class="selection-info">已选择{{ selected.length }}项 </span>
                  <span class="selection-deselect" @click.stop="selected = []">取消选择</span>
                </template>
                <template v-else>
                  文件名
                  <span class="file-count">(共{{ filesStore.files.length }}项)</span>
                  <span class="sort-arrows" :class="sortArrowClass('name')">
                    <NIcon :size="12" class="sort-up" :class="{ on: sortKey === 'name' && sortAsc }">
                      <CaretUpOutline />
                    </NIcon>
                    <NIcon :size="12" class="sort-down" :class="{ on: sortKey === 'name' && !sortAsc }">
                      <CaretDownOutline />
                    </NIcon>
                  </span>
                </template>
              </span>
              <span class="col-size sortable" @click="toggleSort('size')">
                大小
                <span class="sort-arrows" :class="sortArrowClass('size')">
                  <NIcon :size="12" class="sort-up" :class="{ on: sortKey === 'size' && sortAsc }">
                    <CaretUpOutline />
                  </NIcon>
                  <NIcon :size="12" class="sort-down" :class="{ on: sortKey === 'size' && !sortAsc }">
                    <CaretDownOutline />
                  </NIcon>
                </span>
              </span>
              <span class="col-time sortable" @click="toggleSort('time')">
                时间
                <span class="sort-arrows" :class="sortArrowClass('time')">
                  <NIcon :size="12" class="sort-up" :class="{ on: sortKey === 'time' && sortAsc }">
                    <CaretUpOutline />
                  </NIcon>
                  <NIcon :size="12" class="sort-down" :class="{ on: sortKey === 'time' && !sortAsc }">
                    <CaretDownOutline />
                  </NIcon>
                </span>
              </span>
              <span class="col-downloads sortable" @click="toggleSort('downloads')">
                下载
                <span class="sort-arrows" :class="sortArrowClass('downloads')">
                  <NIcon :size="12" class="sort-up" :class="{ on: sortKey === 'downloads' && sortAsc }">
                    <CaretUpOutline />
                  </NIcon>
                  <NIcon :size="12" class="sort-down" :class="{ on: sortKey === 'downloads' && !sortAsc }">
                    <CaretDownOutline />
                  </NIcon>
                </span>
              </span>
            </div>
            <div class="files-scroll" ref="listEl">
              <div
                v-for="file in pageFiles"
                :key="`${file.type}-${file.id}`"
                class="file-row"
                :data-id="file.id"
                :class="{
                  folder: file.type === 'folder',
                  selected: isSelected(file),
                  'drop-target': dropTargetId === file.id && file.type === 'folder' && moveIds.length > 0,
                }"
                @click="onRowClick(file, $event)"
                @contextmenu.prevent="onRowClick(file, $event)"
              >
                <span class="col-check" @click.stop="onCheckClick(file, $event)">
                  <NCheckbox :checked="isSelected(file)" />
                </span>
                <span class="col-name">
                  <NIcon
                    class="file-icon"
                    :size="18"
                    :color="file.type === 'folder' ? 'var(--m3-primary)' : getFileIconColor(file.name)"
                  >
                    <component :is="file.type === 'folder' ? FolderOutline : getFileIconComponent(file.name)" />
                  </NIcon>
                  <span class="file-name">{{ file.name }}</span>
                </span>
                <span class="col-size">{{ file.type === 'folder' ? '—' : file.size || '—' }}</span>
                <span class="col-time">{{ file.time || '—' }}</span>
                <span class="col-downloads">{{ file.downs || '—' }}</span>
              </div>
            </div>
            <div v-if="totalPages > 1" class="files-pager">
              <NPagination v-model:page="page" :page-count="totalPages" />
            </div>
          </div>
        </template>
      </NSpin>
      <!-- 拖拽上传覆盖层（仅覆盖文件列表区域） -->
      <div v-if="dragging && !recycleMode" class="upload-drop-backdrop">
        <div class="upload-drop-hint">
          <NIcon :size="40" :depth="3">
            <CloudUploadOutline />
          </NIcon>
          <NText style="font-size: 14px;">
            拖拽文件到此处上传到: {{ currentFolderName }}
          </NText>
        </div>
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

    <!-- 回收站右键菜单 -->
    <NDropdown
      v-model:show="recycleDropdownShow"
      :options="recycleContextOptions"
      :x="recycleDropdownPos.x"
      :y="recycleDropdownPos.y"
      @select="onRecycleContextSelect"
    />

    <!-- 新建文件夹 -->
    <NModal
      v-model:show="showMkdir"
      preset="dialog"
      title="新建文件夹"
      positive-text="创建"
      negative-text="取消"
      transform-origin="center"
      @positive-click="doMkdir"
    >
      <NInput v-model:value="newFolderName" placeholder="文件夹名称" @keydown.enter="doMkdir" />
    </NModal>

    <!-- 重命名 -->
    <NModal
      v-model:show="showRename"
      preset="dialog"
      title="重命名"
      positive-text="确定"
      negative-text="取消"
      transform-origin="center"
      @positive-click="doRename"
    >
      <NInput v-model:value="renameName" placeholder="新名称" @keydown.enter="doRename" />
    </NModal>

    <!-- 设置访问密码 -->
    <NModal
      v-model:show="showAccess"
      preset="dialog"
      title="设置密码"
      positive-text="确定"
      negative-text="取消"
      transform-origin="center"
      :positive-button-props="{ disabled: accessShows && !accessPwdValid }"
      @positive-click="doSetAccess"
    >
      <div class="access-form">
        <div class="access-row">
          <span class="access-label">访问密码</span>
          <NSwitch v-model:value="accessShows" size="small" />
        </div>
        <NInput v-if="accessShows" v-model:value="accessPwd" placeholder="请输入访问密码" maxlength="6" show-count @keydown.enter="doSetAccess" />
        <p v-if="accessShows && !accessPwdValid" class="access-hint">密码需为 2-6 位且不能包含空格</p>
      </div>
    </NModal>

    <!-- 分享链接 -->
    <NModal
      v-model:show="showShareLink"
      preset="dialog"
      title="分享链接"
      positive-text="复制"
      negative-text="关闭"
      transform-origin="center"
      :on-positive-click="copyShare"
    >
      <div class="share-links">{{ shareLinks }}</div>
      <div v-if="shareQrCodes.length" class="share-qrcodes">
        <div v-for="item in shareQrCodes" :key="item.url" class="share-qrcode-item">
          <img :src="item.dataUrl" :alt="item.name" width="190" height="190" />
        </div>
      </div>
    </NModal>

    <!-- 移动（树形文件夹选择） -->
    <NModal
      v-model:show="showMove"
      preset="dialog"
      title="移动到"
      positive-text="移动"
      negative-text="取消"
      transform-origin="center"
      :positive-button-props="{ disabled: !canMove }"
      @positive-click="doMove"
    >
      <div class="move-tree">
        <NTree
          :data="moveTreeData"
          :selected-keys="moveSelectedKey === null ? [] : [moveSelectedKey]"
          :expanded-keys="moveExpandedKeys"
          :loading-keys="moveLoadingKeys"
          selectable
          block-line
          :render-prefix="() => h(NIcon, { size: 18, color: 'var(--m3-primary)' }, { default: () => h(FolderOutline) })"
          @update:selected-keys="onMoveSelect"
          @update:expanded-keys="(keys: Array<number | string>) => moveExpandedKeys = keys as number[]"
          @load="onMoveLoad"
        />
      </div>
    </NModal>

    <!-- 添加描述 -->
    <NModal
      v-model:show="showDesc"
      preset="dialog"
      title="添加描述"
      positive-text="保存"
      negative-text="取消"
      transform-origin="center"
      @positive-click="doDesc"
    >
      <NInput
        v-model:value="descText"
        type="textarea"
        :autosize="{ minRows: 3, maxRows: 6 }"
        placeholder="文件描述只允许修改一次，建议 300 字数以内。"
        maxlength="300"
        show-count
      />
    </NModal>
  </div>
</template>

<style scoped>
.files-view {
  position: relative;
}

.files-toolbar {
  margin: 0 36px;
  padding: 8px 0;
  display: flex;
  flex-direction: column;
  gap: 8px;
  flex-shrink: 0;
}

.toolbar-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.toolbar-search {
  flex: 1;
  max-width: 300px;
  transition: max-width 0.25s ease;
}

.toolbar-search:focus-within {
  max-width: 100%;
}

.toolbar-search :deep(.n-input) {
  width: 100%;
}

.toolbar-actions {
  display: flex;
  gap: 8px;
  flex-shrink: 0;
  align-items: center;
}

.files-body {
  position: relative;
  flex: 1;
  min-height: 0;
  overflow: hidden;
}

.files-body :deep(.n-spin-container),
.files-body :deep(.n-spin-content) {
  height: 100%;
  display: flex;
  flex-direction: column;
}

.files-empty {
  margin-top: 60px;
}

.upload-drop-backdrop {
  position: absolute;
  inset: 0;
  z-index: 10;
  backdrop-filter: blur(4px);
  background: color-mix(in srgb, var(--m3-surface) 60%, transparent);
  display: flex;
  padding: 0;
}

.upload-drop-hint {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  color: var(--m3-on-surface-variant);
  width: 100%;
  box-sizing: border-box;
  border: 1.3px dashed var(--m3-outline);
  border-radius: 6px;
}

.file-row.folder:hover::before {
  background-color: var(--m3-primary-container);
}

.file-row.drop-target::before {
  background-color: color-mix(in srgb, var(--m3-primary) 18%, transparent);
  inset: 1px;
  border: 1px dashed var(--m3-primary);
}

.file-sortable-ghost {
  opacity: 0.4;
}

.file-sortable-chosen {
  opacity: 0.6;
}

.file-sortable-fallback {
  opacity: 0.9;
  border-radius: 8px;
  box-shadow: 0 4px 16px rgb(0 0 0 / 25%);
  background-color: var(--m3-surface-container-high);
}

.file-row.folder .file-icon {
  color: var(--m3-primary);
}

.col-size {
  width: 90px;
}

.col-time {
  width: 120px;
}

.col-downloads {
  width: 70px;
}

.sortable {
  cursor: pointer;
  user-select: none;
  display: inline-flex;
  align-items: center;
  gap: 4px;
}

.sort-arrows {
  display: inline-flex;
  flex-direction: column;
  line-height: 0;
  gap: 0;
  color: var(--m3-on-surface-variant);
}

.sort-arrows :deep(.n-icon) {
  display: block;
  height: 8px;
  line-height: 0;
  font-size: 12px;
  margin: 0;
  padding: 0;
}

.sort-arrows :deep(.n-icon svg) {
  display: block;
}

.sort-arrows .sort-up,
.sort-arrows .sort-down {
  opacity: 0.35;
  transition: color 0.2s;
}

.sort-arrows .sort-up.on,
.sort-arrows .sort-down.on {
  opacity: 1;
  color: var(--m3-primary);
}

.access-form {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.access-row {
  display: flex;
  align-items: center;
  gap: 12px;
}

.access-label {
  font-size: 13px;
  color: var(--m3-on-surface);
}

.access-hint {
  margin: 0;
  font-size: 12px;
  color: var(--m3-error);
}

.share-links {
  margin: 0;
  white-space: pre-line;
  word-break: break-all;
  font-size: 13px;
  line-height: 1.6;
  max-height: 300px;
  overflow: auto;
}

.share-qrcodes {
  display: flex;
  flex-wrap: wrap;
  gap: 12px;
  margin-top: 12px;
  justify-content: center;
}

.share-qrcode-item {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
}

.share-qrcode-item img {
  border: 1px solid var(--n-border-color, #eee);
  border-radius: 4px;
}

.move-tree {
  max-height: 300px;
  overflow: auto;
}

.files-body.moving :deep(.n-spin-content) {
  opacity: 0.5;
  pointer-events: none;
}
</style>
