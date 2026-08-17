import { computed } from 'vue'
import { darkTheme, type GlobalThemeOverrides } from 'naive-ui'

import { useAppStore } from '@/stores/app'
import { naiveTokens } from '@/shared/colorScheme'

/** naive-ui 主题覆盖，使用 M3 tokens（由配色种子色直接计算，随配色/主题即时更新） */
export function useNaiveTheme() {
  const appStore = useAppStore()

  const theme = computed(() => (appStore.isDark ? darkTheme : undefined))

  const themeOverrides = computed<GlobalThemeOverrides>(() => {
    // 依赖 colorScheme / themeSource：配色或主题变化时重算
    void appStore.colorScheme
    void appStore.themeSource
    const t = naiveTokens(appStore.colorScheme, appStore.isDark)
    return {
      common: {
        fontFamily: getComputedStyle(document.documentElement).getPropertyValue('--font-family').trim(),
        primaryColor: t.primary,
        primaryColorHover: t.primary,
        primaryColorPressed: t.primary,
        primaryColorSuppl: t.primary,
        successColor: t.success,
        infoColor: t.info,
        warningColor: t.warning,
        errorColor: t.error,
        bodyColor: t.body,
        cardColor: t.card,
        modalColor: t.modal,
        popoverColor: t.popover,
        dividerColor: t.outlineVariant,
        borderColor: t.outlineVariant,
        borderRadius: '6px',
      },
      Divider: {
        color: t.outlineVariant,
      },
    }
  })

  return { theme, themeOverrides }
}
