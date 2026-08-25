<script setup lang="ts">
import { computed, h, onMounted, ref, watch, type Component } from 'vue'

import {
  NForm,
  NFormItem,
  NDivider,
  NRadioGroup,
  NRadioButton,
  NSwitch,
  NInputNumber,
  NButton,
  NButtonGroup,
  NInput,
  NInputGroup,
  NSelect,
  NTag,
  NTooltip,
  NIcon,
  useDialog,
  useMessage,
} from 'naive-ui'

import {
  CloudDownloadOutline,
  CopyOutline,
  DocumentTextOutline,
  FolderOpenOutline,
  FolderOutline,
  HeartOutline,
  LogInOutline,
  LogoGithub,
  LogOutOutline,
  RefreshOutline,
  RocketOutline,
  ReaderOutline,
  TrashOutline,
} from '@vicons/ionicons5'
import ViewHeader from '@/components/layout/ViewHeader.vue'
import { useAppStore } from '@/stores/app'
import { usePreferenceStore } from '@/stores/preference'
import { useUploadTrafficStore } from '@/stores/uploadTraffic'
import { useUpdateCheck } from '@/composables/useUpdateCheck'
import type { AppConfig, ThemeSource } from '@/shared/types'
import { COLOR_SCHEMES, DEFAULT_COLOR_SCHEME } from '@/shared/colorScheme'
import {
  ABOUT_LINKS,
  DEFAULT_CONFIG,
  DEFAULT_USER_AGENT,
  LOG_LEVELS,
  USER_AGENT_PRESETS,
  VERSION,
} from '@/shared/constants'
import { formatSize, hasUnsafeHeaderChars, parseSizeText, sanitizeHeaderValue } from '@/shared/util'
import { open as openDialog } from '@tauri-apps/plugin-dialog'
import { openPath, openUrl, revealItemInDir } from '@tauri-apps/plugin-opener'
import { configClear, configReset, configSet, logClear, logGetFile, lanzouProfile } from '@/shared/api'

const appStore = useAppStore()
const preferenceStore = usePreferenceStore()
const trafficStore = useUploadTrafficStore()
const message = useMessage()
const dialog = useDialog()
const { manualCheck } = useUpdateCheck()

/** 关于页链接图标映射 */
const ABOUT_LINK_ICONS: Record<string, Component> = {
  LogoGithub,
  RocketOutline,
  DocumentTextOutline,
  HeartOutline,
}

/** 上次检查更新时间展示文案 */
const lastCheckTimeText = computed(() => {
  const t = preferenceStore.config.lastCheckUpdateTime
  return t ? new Date(t).toLocaleString() : '从未检查'
})

async function onOpenLink(url: string) {
  try {
    await openUrl(url)
  } catch (e) {
    message.error(e instanceof Error ? e.message : String(e))
  }
}

/** 今日累计上传流量（字节） */
const todayTraffic = computed(() => trafficStore.todaySize())

const username = ref('')
const password = ref('')
// 允许清空输入值并显示占位符，重新输入后再持久化。
const downloadMax = ref<number | null>(null)
const uploadMax = ref<number | null>(null)
const uploadWarningSize = ref<number | null>(null)
const splitSize = ref<number | null>(null)
/** 强制重挂载分片输入框（超限回退时让显示值复位） */
const splitInputKey = ref(0)
/** 本次编辑开始前的分片大小，用于超限回滚 */
const splitEditStart = ref<number | null>(null)
/** 当前日志文件路径 */
const logPath = ref('')

/** User-Agent 编辑缓冲，失焦时持久化 */
const userAgent = ref(DEFAULT_USER_AGENT)

/** UA 是否含非法头字符（CR/LF） */
const uaHasIssue = computed(() => hasUnsafeHeaderChars(userAgent.value))

const logLevelOptions = LOG_LEVELS.map((level) => ({ label: level, value: level }))

onMounted(async () => {
  try {
    await preferenceStore.load()
    downloadMax.value = preferenceStore.config.downloadMax ?? null
    uploadMax.value = preferenceStore.config.uploadMax ?? null
    uploadWarningSize.value = preferenceStore.config.uploadWarningSize ?? null
    splitSize.value = preferenceStore.config.splitSize ?? null
    userAgent.value = preferenceStore.config.userAgent ?? DEFAULT_USER_AGENT
  } catch (e) {
    console.warn('load preference failed', e)
  }
  try {
    logPath.value = await logGetFile()
  } catch (e) {
    console.warn('load log path failed', e)
  }
})

async function onDownloadMax(v: number | null) {
  downloadMax.value = v
  if (v != null) await onConfigChange({ downloadMax: v })
}
async function onUploadMax(v: number | null) {
  uploadMax.value = v
  if (v != null) await onConfigChange({ uploadMax: v })
}
async function onWarningSize(v: number | null) {
  uploadWarningSize.value = v
  if (v != null) {
    await onConfigChange({ uploadWarningSize: v })
  } else {
    // 清空警戒线即关闭：本地置空并从后端删除该键
    preferenceStore.config = { ...preferenceStore.config, uploadWarningSize: undefined }
    try {
      await configClear(['upload_warning_size'])
    } catch (e) {
      message.error(e instanceof Error ? e.message : String(e))
    }
  }
}

// 文件分片大小仅更新本地显示，失焦时校验账号限制；超限时提示并回滚。
async function onSplitInput(v: number | null) {
  splitSize.value = v
}

function onSplitFocus() {
  splitEditStart.value = splitSize.value
}

async function onSplitBlur() {
  const v = splitSize.value
  if (v == null || v === splitEditStart.value) return
  if (!appStore.isLoggedIn) {
    message.warning('请先登录')
    splitSize.value = splitEditStart.value
    splitInputKey.value++
    return
  }
  try {
    const profile = await lanzouProfile()
    const limitBytes = profile.maxSize ? parseSizeText(profile.maxSize) : null
    if (limitBytes && v * 1024 ** 2 > limitBytes) {
      message.error(`文件分片大小不应大于账号限制的单个文件大小（${formatSize(limitBytes)}）`)
      splitSize.value = splitEditStart.value
      splitInputKey.value++
      return
    }
    await onConfigChange({ splitSize: v })
  } catch (e) {
    message.error(e instanceof Error ? e.message : String(e))
    splitSize.value = splitEditStart.value
    splitInputKey.value++
  }
}

async function pickDownloadDir() {
  const picked = await openDialog({ directory: true })
  if (picked) {
    preferenceStore.config.downloadDir = picked
    try {
      await configSet({ downloadDir: picked })
      message.success('下载位置已更新')
    } catch (e) {
      message.error(e instanceof Error ? e.message : String(e))
    }
  }
}

async function openDownloadDir() {
  const dir = preferenceStore.config.downloadDir
  if (!dir) {
    message.warning('请先设置下载位置')
    return
  }
  try {
    await openPath(dir)
  } catch (e) {
    message.error(e instanceof Error ? e.message : String(e))
  }
}

const THEME_LABELS: Record<ThemeSource, string> = {
  light: '浅色',
  dark: '深色',
  auto: '跟随主题',
}

async function onThemeChange(value: ThemeSource) {
  await appStore.setThemeSource(value)
  message.success(`主题已切换为 ${THEME_LABELS[value]}`)
}

// 轻量模式依赖"关闭时最小化到托盘"
watch(
  () => preferenceStore.config.lightweightMode,
  (enabled) => {
    if (enabled && !preferenceStore.config.minimizeToTrayOnClose) {
      preferenceStore.config.minimizeToTrayOnClose = true
      configSet({ minimizeToTrayOnClose: true }).catch(() => {})
    }
  },
)
watch(
  () => preferenceStore.config.minimizeToTrayOnClose,
  (enabled) => {
    if (!enabled && preferenceStore.config.lightweightMode) {
      preferenceStore.config.lightweightMode = false
      configSet({ lightweightMode: false }).catch(() => {})
    }
  },
)

async function onConfigChange(patch: Partial<AppConfig>) {
  preferenceStore.config = { ...preferenceStore.config, ...patch }
  try {
    await configSet(patch)
  } catch (e) {
    message.error(e instanceof Error ? e.message : String(e))
  }
}

/** 持久化 UA 并同步后端（立即生效）；返回是否已写入 */
async function persistUserAgent(value: string): Promise<boolean> {
  const v = value.trim()
  if (!v) {
    message.warning('User-Agent 不能为空')
    return false
  }
  if (v === preferenceStore.config.userAgent) return true
  preferenceStore.config = { ...preferenceStore.config, userAgent: v }
  try {
    await configSet({ userAgent: v })
    message.success('User-Agent 已更新')
    return true
  } catch (e) {
    userAgent.value = preferenceStore.config.userAgent ?? DEFAULT_USER_AGENT
    message.error(e instanceof Error ? e.message : String(e))
    return false
  }
}

/** User-Agent 失焦保存：先清理非法字符再校验非空 */
async function onUserAgentBlur() {
  const v = sanitizeHeaderValue(userAgent.value)
  userAgent.value = v
  if (!v) {
    message.warning('User-Agent 不能为空')
    userAgent.value = preferenceStore.config.userAgent ?? DEFAULT_USER_AGENT
    return
  }
  await persistUserAgent(v)
}

/** 点击预设浏览器 UA 并立即生效 */
async function onUserAgentPreset(value: string) {
  userAgent.value = value
  await persistUserAgent(value)
}

/** 一键清理 UA 中的非法头字符（CR/LF） */
async function cleanUserAgent() {
  userAgent.value = sanitizeHeaderValue(userAgent.value)
  await persistUserAgent(userAgent.value)
}

/** 恢复默认 User-Agent 并立即生效 */
async function onUserAgentReset() {
  userAgent.value = DEFAULT_USER_AGENT
  if (preferenceStore.config.userAgent === DEFAULT_USER_AGENT) {
    message.info('已是最新默认值')
    return
  }
  await persistUserAgent(DEFAULT_USER_AGENT)
}

async function copyLogPath() {
  if (!logPath.value) return
  try {
    await navigator.clipboard.writeText(logPath.value)
    message.success('日志路径已复制')
  } catch (e) {
    message.error(e instanceof Error ? e.message : String(e))
  }
}

async function revealLogPath() {
  if (!logPath.value) return
  try {
    await revealItemInDir(logPath.value)
  } catch (e) {
    message.error(e instanceof Error ? e.message : String(e))
  }
}

async function onLogLevelChange(v: string) {
  preferenceStore.config = { ...preferenceStore.config, logLevel: v }
  try {
    await configSet({ logLevel: v })
    message.success('日志级别已保存，重启后生效')
  } catch (e) {
    message.error(e instanceof Error ? e.message : String(e))
  }
}

function onClearLog() {
  dialog.warning({
    title: '清空日志',
    content: '确定清空当前日志文件吗？',
    positiveText: '清空',
    negativeText: '取消',
    transformOrigin: 'center',
    onPositiveClick: async () => {
      try {
        await logClear()
        message.success('日志已清空')
      } catch (e) {
        message.error(e instanceof Error ? e.message : String(e))
      }
    },
  })
}

async function onLogin() {
  if (!username.value.trim() || !password.value) {
    message.warning('请输入账号和密码')
    return
  }
  const result = await appStore.login(username.value.trim(), password.value)
  if (result.ok) {
    message.success('登录成功')
    password.value = ''
  } else {
    message.error(result.message || '登录失败')
  }
}

function onLogout() {
  dialog.warning({
    title: '退出登录',
    content: '确定退出当前账号吗？',
    positiveText: '退出',
    negativeText: '取消',
    transformOrigin: 'center',
    onPositiveClick: async () => {
      await appStore.logout()
      message.success('已退出登录')
    },
  })
}

function showDisclaimer() {
  const lines = [
    '本项目仅供个人学习和技术研究使用。',
    '• 使用限制：禁止将本项目用于任何违法行为，请遵守蓝奏云服务条款及相关法律法规。',
    '• 责任声明：你应了解相应的风险，因使用本项目产生的任何法律纠纷或损失，均由使用者自行承担。',
    '• 争议处理：如权利方认为本项目侵犯其权益，请通过 Issues 联系，我们将积极配合处理。',
  ]
  dialog.info({
    title: '免责声明',
    content: () => h('div', { style: 'white-space: pre-wrap; line-height: 1.8' }, lines.join('\n')),
    positiveText: '我知道了',
    transformOrigin: 'center',
  })
}

/** 恢复默认设置：重置全部配置为默认值，保留登录状态（接口地址/域名/UA/cookies） */
function onRestoreDefaults() {
  dialog.error({
    title: '恢复默认设置',
    content: '将重置全部设置项为默认值，但保留登录状态，是否恢复？',
    positiveText: '恢复',
    negativeText: '取消',
    transformOrigin: 'center',
    onPositiveClick: async () => {
      try {
        const cfg = await configReset()
        preferenceStore.config = { ...DEFAULT_CONFIG, ...cfg }
        downloadMax.value = cfg.downloadMax ?? null
        uploadMax.value = cfg.uploadMax ?? null
        uploadWarningSize.value = cfg.uploadWarningSize ?? null
        splitSize.value = cfg.splitSize ?? null
        appStore.themeSource = 'auto'
        appStore.colorScheme = DEFAULT_COLOR_SCHEME
        appStore.applyTheme()
        message.success('已恢复默认设置')
      } catch (e) {
        message.error(e instanceof Error ? e.message : String(e))
      }
    },
  })
}
</script>

<template>
  <div class="preference-view">
    <ViewHeader title="设置" />
    <div class="panel-body">
      <div class="preference-form-wrapper">
        <div class="preference-form-scroll">
          <NForm
            label-placement="left"
            label-align="left"
            label-width="260px"
            size="small"
            class="form-preference"
            @submit.prevent
          >
            <NDivider title-placement="left">账号</NDivider>
            <NFormItem label="登录状态">
              <NTag v-if="appStore.isLoggedIn" type="success" round>已登录</NTag>
              <NTag v-else type="warning" round>未登录</NTag>
            </NFormItem>
            <template v-if="appStore.isLoggedIn">
              <NFormItem label="手机号">
                <span>{{ appStore.profile?.verification || '—' }}</span>
              </NFormItem>
              <NFormItem label="最近登录">
                <span>{{ appStore.profile?.lastLogin || '—' }}</span>
              </NFormItem>
              <NFormItem label="操作">
                <NButton type="error" ghost size="small" @click="onLogout">
                  <template #icon>
                    <NIcon :size="16">
                      <LogOutOutline />
                    </NIcon>
                  </template>
                  退出登录
                </NButton>
              </NFormItem>
            </template>
            <template v-else>
              <NFormItem label="账号">
                <NInput v-model:value="username" placeholder="请输入账号" class="pref-login-input" />
              </NFormItem>
              <NFormItem label="密码">
                <NInput
                  v-model:value="password"
                  type="password"
                  show-password-on="click"
                  placeholder="请输入密码"
                  class="pref-login-input"
                  @keydown.enter="onLogin"
                />
              </NFormItem>
              <NFormItem label="操作">
                <NButton type="primary" ghost size="small" :loading="appStore.loginLoading" @click="onLogin">
                  <template #icon>
                    <NIcon :size="16">
                      <LogInOutline />
                    </NIcon>
                  </template>
                  登录
                </NButton>
              </NFormItem>
            </template>

            <NDivider title-placement="left">外观</NDivider>
            <NFormItem label="主题">
              <NRadioGroup :value="appStore.themeSource" size="small" @update:value="onThemeChange">
                <NRadioButton value="light">浅色</NRadioButton>
                <NRadioButton value="dark">深色</NRadioButton>
                <NRadioButton value="auto">跟随系统</NRadioButton>
              </NRadioGroup>
            </NFormItem>
            <NFormItem label="配色方案">
              <div class="color-scheme-picker">
                <NTooltip v-for="scheme in COLOR_SCHEMES" :key="scheme.id">
                  <template #trigger>
                    <button
                      type="button"
                      class="scheme-swatch"
                      :class="{ active: appStore.colorScheme === scheme.id }"
                      :style="{ background: scheme.seed }"
                      :aria-label="scheme.label"
                      @click="appStore.setColorScheme(scheme.id)"
                    />
                  </template>
                  {{ scheme.label }}
                </NTooltip>
              </div>
            </NFormItem>

            <NDivider title-placement="left">传输</NDivider>
            <NFormItem label="下载位置">
              <NInputGroup class="pref-path-group">
                <NInput :value="preferenceStore.downloadDir || '未设置'" readonly class="pref-control-full" />
                <NTooltip placement="top">
                  <template #trigger>
                    <NButton class="pref-icon-button" @click="pickDownloadDir">
                      <template #icon>
                        <NIcon :size="16">
                          <FolderOpenOutline />
                        </NIcon>
                      </template>
                    </NButton>
                  </template>
                  选择下载位置
                </NTooltip>
                <NTooltip placement="top">
                  <template #trigger>
                    <NButton class="pref-icon-button" @click="openDownloadDir">
                      <template #icon>
                        <NIcon :size="16">
                          <FolderOutline />
                        </NIcon>
                      </template>
                    </NButton>
                  </template>
                  打开下载目录
                </NTooltip>
              </NInputGroup>
            </NFormItem>
            <NFormItem label="同时上传数">
              <NInputNumber
                v-model:value="uploadMax"
                button-placement="both"
                :min="1"
                :max="3"
                placeholder="请输入"
                class="pref-number"
                @update:value="onUploadMax"
                @keydown.enter="$event.target.blur()"
              />
            </NFormItem>
            <NFormItem label="同时下载数">
              <NInputNumber
                v-model:value="downloadMax"
                button-placement="both"
                :min="1"
                :max="5"
                placeholder="请输入"
                class="pref-number"
                @update:value="onDownloadMax"
                @keydown.enter="$event.target.blur()"
              />
            </NFormItem>
            <NFormItem label="上传流量警戒">
              <div class="pref-inline-row">
                <NInputNumber
                  v-model:value="uploadWarningSize"
                  button-placement="both"
                  :min="1"
                  :max="1024"
                  placeholder="请输入"
                  class="pref-number"
                  @update:value="onWarningSize"
                  @keydown.enter="$event.target.blur()"
                />
                <span class="pref-inline-row__meta">GB</span>
                <span class="pref-inline-row__meta pref-inline-row__meta--muted"
                  >今日流量 {{ formatSize(todayTraffic) }}</span
                >
              </div>
            </NFormItem>
            <NFormItem label="文件分片大小">
              <div class="pref-inline-row">
                <NInputNumber
                  button-placement="both"
                  :key="splitInputKey"
                  :value="splitSize"
                  :min="1"
                  :max="10240"
                  placeholder="请输入"
                  class="pref-number"
                  @update:value="onSplitInput"
                  @focus="onSplitFocus"
                  @blur="onSplitBlur"
                  @keydown.enter="$event.target.blur()"
                />
                <span class="pref-inline-row__meta">MB</span>
                <span class="pref-inline-row__meta pref-inline-row__meta--muted">不应超过账号限制</span>
              </div>
            </NFormItem>

            <NDivider title-placement="left">日志</NDivider>
            <NFormItem label="日志文件">
              <NInputGroup class="pref-path-group">
                <NInput :value="logPath || '暂无日志文件'" readonly class="pref-control-full" />
                <NTooltip placement="top">
                  <template #trigger>
                    <NButton class="pref-icon-button" @click="copyLogPath">
                      <template #icon>
                        <NIcon :size="14">
                          <CopyOutline />
                        </NIcon>
                      </template>
                    </NButton>
                  </template>
                  复制日志路径
                </NTooltip>
                <NTooltip placement="top">
                  <template #trigger>
                    <NButton class="pref-icon-button" @click="revealLogPath">
                      <template #icon>
                        <NIcon :size="14">
                          <FolderOpenOutline />
                        </NIcon>
                      </template>
                    </NButton>
                  </template>
                  在文件夹中显示
                </NTooltip>
              </NInputGroup>
            </NFormItem>
            <NFormItem label="日志级别">
              <div class="pref-inline-row">
                <NSelect
                  :value="preferenceStore.config.logLevel"
                  :options="logLevelOptions"
                  class="pref-control-auto pref-control-log-level"
                  @update:value="onLogLevelChange"
                />
                <span class="pref-inline-row__meta pref-inline-row__meta--muted">重启后生效</span>
              </div>
            </NFormItem>
            <NFormItem label=" ">
              <NButton type="error" ghost size="small" @click="onClearLog">
                <template #icon>
                  <NIcon>
                    <TrashOutline />
                  </NIcon>
                </template>
                清空日志
              </NButton>
            </NFormItem>

            <NDivider title-placement="left">通用</NDivider>
            <NFormItem label="用户代理">
              <div class="ua-field-wrapper">
                <NInput
                  v-model:value="userAgent"
                  type="textarea"
                  :autosize="{ minRows: 2, maxRows: 4 }"
                  placeholder="User-Agent"
                  @blur="onUserAgentBlur"
                />
                <div v-if="uaHasIssue" class="ua-warn-bar">
                  <span class="ua-warn-text">检测到换行等非法字符，可能触发服务器拦截</span>
                  <NButton size="small" type="primary" ghost @click="cleanUserAgent">清理</NButton>
                </div>
              </div>
            </NFormItem>
            <NFormItem label=" ">
              <div class="ua-preset-row">
                <NButtonGroup size="small">
                  <NButton
                    v-for="preset in USER_AGENT_PRESETS"
                    :key="preset.label"
                    @click="onUserAgentPreset(preset.value)"
                  >
                    {{ preset.label }}
                  </NButton>
                </NButtonGroup>
                <NButton type="error" size="small" ghost @click="onUserAgentReset">重置</NButton>
              </div>
            </NFormItem>
            <NFormItem label="关闭时最小化到托盘">
              <NSwitch
                :value="!!preferenceStore.config.minimizeToTrayOnClose"
                @update:value="(v) => onConfigChange({ minimizeToTrayOnClose: v })"
              />
            </NFormItem>
            <NFormItem>
              <template #label>
                <span class="pref-hint-label">
                  <span class="pref-hint-label__title">轻量模式</span>
                  <span class="pref-hint-label__hint">最小化到托盘时销毁渲染进程，降低内存占用</span>
                </span>
              </template>
              <NSwitch
                :value="!!preferenceStore.config.lightweightMode"
                @update:value="(v) => onConfigChange({ lightweightMode: v })"
              />
            </NFormItem>
            <NFormItem label="开发者工具">
              <NSwitch
                :value="!!preferenceStore.config.devTools"
                @update:value="(v) => onConfigChange({ devTools: v })"
              />
            </NFormItem>

            <NDivider title-placement="left">更新</NDivider>
            <NFormItem label="启动时检查更新">
              <NSwitch
                :value="!!preferenceStore.config.autoCheckUpdate"
                @update:value="(v) => onConfigChange({ autoCheckUpdate: v })"
              />
            </NFormItem>
            <NFormItem label="接收测试版更新">
              <NSwitch
                :value="!!preferenceStore.config.betaUpdate"
                @update:value="(v) => onConfigChange({ betaUpdate: v })"
              />
            </NFormItem>
            <NFormItem label="GitHub 加速地址">
              <NInput
                :value="preferenceStore.config.githubProxyUrl ?? ''"
                placeholder="留空则直连 GitHub"
                style="width: 360px"
                @update:value="(v) => onConfigChange({ githubProxyUrl: v ?? undefined })"
                @keydown.enter="($event.target as HTMLInputElement)?.blur()"
              />
            </NFormItem>
            <NFormItem label="上次检查时间">
              <div class="pref-inline-row">
                <span>{{ lastCheckTimeText }}</span>
              </div>
            </NFormItem>
            <NFormItem label=" ">
                <NButton size="small" @click="manualCheck">
                  <template #icon>
                    <NIcon :size="14">
                      <CloudDownloadOutline />
                    </NIcon>
                  </template>
                  立即检查
                </NButton>
            </NFormItem>

            <NDivider title-placement="left">关于</NDivider>
            <NFormItem label="版本">
              <span>v{{ VERSION }}</span>
            </NFormItem>
            <NFormItem label="链接">
              <div class="about-link-row">
                <NButton v-for="link in ABOUT_LINKS" :key="link.label" size="small" @click="onOpenLink(link.url)">
                  <template #icon>
                    <NIcon :size="16">
                      <component :is="ABOUT_LINK_ICONS[link.icon]" />
                    </NIcon>
                  </template>
                  {{ link.label }}
                </NButton>
              </div>
            </NFormItem>
            <NFormItem label="免责声明">
              <NButton size="small" @click="showDisclaimer">
                <template #icon>
                  <NIcon :size="14">
                    <ReaderOutline />
                  </NIcon>
                </template>
                点击查看
              </NButton>
            </NFormItem>
            <NFormItem label="">
              <NButton type="error" ghost size="small" @click="onRestoreDefaults">
                <template #icon>
                  <NIcon>
                    <RefreshOutline />
                  </NIcon>
                </template>
                恢复默认设置
              </NButton>
            </NFormItem>
          </NForm>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.panel-body {
  flex: 1;
  min-width: 0;
  overflow: hidden;
}

.pref-hint-label {
  display: inline-flex;
  flex-direction: column;
  gap: 2px;
  padding-bottom: 22px;
  line-height: 1.35;
}

.pref-hint-label__title {
  display: block;
}

.pref-hint-label__hint {
  display: block;
  color: var(--m3-on-surface-variant);
  font-size: 12px;
  font-weight: 400;
  white-space: normal;
}

.color-scheme-picker {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}

.scheme-swatch {
  width: 26px;
  height: 26px;
  border-radius: 50%;
  border: 2px solid transparent;
  cursor: pointer;
  padding: 0;
  transition:
    transform 0.15s,
    border-color 0.15s;
}

.scheme-swatch:hover {
  transform: scale(1.1);
}

.scheme-swatch.active {
  border-color: var(--m3-on-surface);
  box-shadow: 0 0 0 2px var(--m3-primary);
}

.about-link-row {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.ua-field-wrapper {
  display: flex;
  flex-direction: column;
  width: 100%;
  max-width: 480px;
  min-width: 260px;
}

.ua-warn-bar {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 12px;
  margin-top: 6px;
  border-radius: 6px;
  background: color-mix(in srgb, var(--m3-warning) 14%, transparent);
}

.ua-warn-text {
  flex: 1;
  font-size: 13px;
  color: var(--m3-on-surface);
}

.ua-preset-row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
}

.pref-number :deep(.n-input__input-el) {
  text-align: center;
}
</style>
