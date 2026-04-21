<script setup lang="ts">
// This starter template is using Vue 3 <script setup> SFCs
// Check out https://vuejs.org/api/sfc-script-setup.html#script-setup
import AppContent from './AppContent.vue'
import { GlobalThemeOverrides } from 'naive-ui'

const themeOverrides: GlobalThemeOverrides = {
  common: {
    primaryColor: '#7A95EF',
    primaryColorHover: '#A9BFFC',
    primaryColorPressed: '#5D72C9',
    primaryColorSuppl: '#A9BFFC',
    borderRadius: '4px',
    borderRadiusSmall: '3px',
    heightMedium: '32px',
  },
  Button: {
    paddingSmall: '0 8px',
    paddingMedium: '0 12px',
  },
  Dropdown: {
    borderRadius: '5px',
    padding: '6px 2px',
    optionColorHover: '#7A95EF',
    optionTextColorHover: '#FFF',
    optionHeightMedium: '28px',
  },
}
</script>

<template>
  <n-config-provider :theme-overrides="themeOverrides">
    <n-modal-provider>
      <n-notification-provider placement="bottom-right" :max="3">
        <n-message-provider>
          <AppContent />
        </n-message-provider>
      </n-notification-provider>
    </n-modal-provider>
  </n-config-provider>
</template>
