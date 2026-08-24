import { computed, ref, watch } from 'vue'
import { defineStore } from 'pinia'

import { listen } from '@tauri-apps/api/event'

import type { Profile, ThemeSource } from '@/shared/types'
import { lanzouLogin, lanzouLogout, lanzouProfile } from '@/shared/api'
import { applyColorScheme, DEFAULT_COLOR_SCHEME } from '@/shared/colorScheme'
import { usePreferenceStore } from '@/stores/preference'

export const useAppStore = defineStore('app', () => {
  const preferenceStore = usePreferenceStore()
  const themeSource = ref<ThemeSource>('auto')
  const colorScheme = ref(DEFAULT_COLOR_SCHEME)
  const isLoggedIn = ref(false)
  const profile = ref<Profile | null>(null)
  const loginLoading = ref(false)
  let loginListenerReady = false

  const mql = window.matchMedia('(prefers-color-scheme: dark)')
  const systemDark = ref(mql.matches)
  mql.addEventListener('change', (e) => {
    systemDark.value = e.matches
  })

  const isDark = computed(() => {
    if (themeSource.value === 'auto') {
      return systemDark.value
    }
    return themeSource.value === 'dark'
  })

  watch(isDark, () => {
    applyTheme()
  })

  function applyTheme() {
    const root = document.documentElement
    if (isDark.value) {
      root.setAttribute('data-theme', 'dark')
    } else {
      root.removeAttribute('data-theme')
    }
    applyColorScheme(colorScheme.value)
  }

  async function loadConfig() {
    try {
      const cfg = preferenceStore.config
      if (cfg.themeSource) themeSource.value = cfg.themeSource
      if (cfg.colorScheme) colorScheme.value = cfg.colorScheme
      if (cfg.cookies?.length) isLoggedIn.value = true
    } catch (e) {
      console.warn('load config failed', e)
    } finally {
      applyTheme()
    }
  }

  async function setThemeSource(value: ThemeSource) {
    themeSource.value = value
    applyTheme()
    try {
      await preferenceStore.update({ themeSource: value })
    } catch (e) {
      console.warn('save theme failed', e)
    }
  }

  async function setColorScheme(value: string) {
    colorScheme.value = value
    applyTheme()
    try {
      await preferenceStore.update({ colorScheme: value })
    } catch (e) {
      console.warn('save color scheme failed', e)
    }
  }

  /** 账号密码直接登录 */
  async function login(username: string, password: string) {
    loginLoading.value = true
    try {
      profile.value = await lanzouLogin(username, password)
      isLoggedIn.value = true
      return { ok: true as const }
    } catch (e) {
      return { ok: false as const, message: e instanceof Error ? e.message : String(e) }
    } finally {
      loginLoading.value = false
    }
  }

  async function refreshProfile() {
    try {
      const p = await lanzouProfile()
      profile.value = p
      if (!p.isLogin) isLoggedIn.value = false
    } catch (e) {
      console.warn('load profile failed', e)
    }
  }

  async function logout() {
    try {
      await lanzouLogout()
    } catch (e) {
      console.warn('logout failed', e)
    }
    isLoggedIn.value = false
    profile.value = null
  }

  /** 订阅 Rust 端登录成功事件 */
  async function setupLoginListener() {
    if (loginListenerReady) return
    await listen('login:success', () => {
      isLoggedIn.value = true
      void refreshProfile()
    })
    loginListenerReady = true
  }

  return {
    themeSource,
    colorScheme,
    isLoggedIn,
    profile,
    loginLoading,
    isDark,
    applyTheme,
    loadConfig,
    setThemeSource,
    setColorScheme,
    login,
    refreshProfile,
    logout,
    setupLoginListener,
  }
})
