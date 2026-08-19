<script setup lang="ts">
defineProps<{ title: string }>()
</script>

<template>
  <header class="panel-header">
    <h4>{{ title }}</h4>
    <div class="header-extra">
      <slot />
    </div>
  </header>
</template>

<style scoped>
.panel-header {
  position: relative;
  padding: var(--header-top-offset) 0 12px;
  margin: 0 36px;
  border-bottom: 2px solid var(--panel-border);
  user-select: none;
  flex-shrink: 0;
}

.panel-header h4 {
  margin: 0;
  color: var(--panel-title);
  font-size: 16px;
  font-weight: normal;
  line-height: 24px;
}

.header-extra {
  position: absolute;
  right: 0;
  top: calc(var(--header-top-offset) + 12px);
  transform: translateY(-50%);
  display: flex;
  align-items: center;
}
</style>
