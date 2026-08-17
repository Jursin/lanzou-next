<script setup lang="ts">
import { onMounted } from 'vue'

import { useUpdateCheck } from '@/composables/useUpdateCheck'

const { silentStartupCheck } = useUpdateCheck()

onMounted(() => {
  void silentStartupCheck()
})
</script>

<template></template>
