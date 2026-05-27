<script setup lang="tsx">
import { commands } from './bindings.ts'
import LoginDialog from './dialogs/LoginDialog.vue'
import ProgressesPane from './panes/ProgressesPane/ProgressesPane.vue'
import SearchPane from './panes/SearchPane.vue'
import ChapterPane from './panes/ChapterPane/ChapterPane.vue'
import FavoritePane from './panes/FavoritePane.vue'
import DownloadedPane from './panes/DownloadedPane/DownloadedPane.vue'
import LogDialog from './dialogs/LogDialog.vue'
import AboutDialog from './dialogs/AboutDialog.vue'
import SettingsDialog from './dialogs/SettingsDialog.vue'
import { onMounted, ref, watch } from 'vue'
import { NAvatar, NButton, NIcon, NInput, NInputGroup, NInputGroupLabel, NTabPane, NTabs, useMessage } from 'naive-ui'
import { useStore } from './store.ts'
import { PhClockCounterClockwise, PhGearSix, PhInfo, PhUser } from '@phosphor-icons/vue'

const store = useStore()

const message = useMessage()

const logDialogShowing = ref<boolean>(false)
const loginDialogShowing = ref<boolean>(false)
const settingsDialogShowing = ref<boolean>(false)
const aboutDialogShowing = ref<boolean>(false)

watch(
  () => store.config,
  async () => {
    if (store.config === undefined) {
      return
    }
    await commands.saveConfig(store.config)
    message.success('保存配置成功')
  },
  { deep: true },
)

watch(
  () => store.config?.cookie,
  async (value, oldValue) => {
    if (store.config === undefined) {
      return
    }
    if (oldValue !== undefined && oldValue !== '' && value === '') {
      // 如果旧的 cookie 不为空，新的 cookie 为空，相当于退出登录
      store.userProfile = undefined
      store.config.cookie = ''
      message.success('已退出登录')
      return
    } else if (value === undefined || value === '') {
      // 如果 cookie 为空，说明用户没有登录
      return
    }

    const result = await commands.getUserProfile()
    if (result.status === 'error') {
      console.error(result.error)
      store.userProfile = undefined
      return
    }
    store.userProfile = result.data
    message.success('获取用户信息成功')
  },
)

onMounted(async () => {
  // 屏蔽浏览器右键菜单
  document.oncontextmenu = (event) => {
    event.preventDefault()
  }
  // 获取配置
  store.config = await commands.getConfig()
})
</script>

<template>
  <div v-if="store.config !== undefined" class="h-screen flex flex-col">
    <div class="flex gap-1 pt-2 px-2">
      <n-input-group>
        <n-input-group-label>Cookie</n-input-group-label>
        <n-input v-model:value="store.config.cookie" placeholder="手动输入或点击右侧的按钮登录" clearable />
        <n-button type="primary" @click="loginDialogShowing = true">
          <template #icon>
            <n-icon size="20">
              <PhUser />
            </n-icon>
          </template>
          登录
        </n-button>
      </n-input-group>

      <div v-if="store.userProfile !== undefined" class="flex items-center">
        <n-avatar :src="store.userProfile.avatar" round />
        <span class="whitespace-nowrap">{{ store.userProfile.username }}</span>
      </div>
    </div>

    <div class="flex flex-1 overflow-hidden">
      <n-tabs class="h-full w-1/2" v-model:value="store.currentTabName" type="line" size="small" animated>
        <n-tab-pane class="h-full overflow-auto p-0!" name="search" tab="漫画搜索" display-directive="show">
          <SearchPane />
        </n-tab-pane>
        <n-tab-pane class="h-full overflow-auto p-0!" name="favorite" tab="漫画收藏" display-directive="show">
          <FavoritePane />
        </n-tab-pane>
        <n-tab-pane class="h-full overflow-auto p-0!" name="downloaded" tab="本地库存" display-directive="show">
          <DownloadedPane />
        </n-tab-pane>
        <n-tab-pane class="h-full overflow-auto p-0!" name="chapter" tab="章节详情" display-directive="show">
          <ChapterPane />
        </n-tab-pane>
      </n-tabs>

      <div class="w-1/2 overflow-auto flex flex-col">
        <div
          class="flex min-h-8.5 gap-col-1 mx-2 items-center border-solid border-0 border-b box-border border-[rgb(239,239,245)]">
          <div class="text-xl font-bold box-border">下载列表</div>
          <div class="flex-1" />
          <n-button size="small" @click="logDialogShowing = true">
            <template #icon>
              <n-icon size="20">
                <PhClockCounterClockwise />
              </n-icon>
            </template>
            日志
          </n-button>
          <n-button size="small" @click="settingsDialogShowing = true">
            <template #icon>
              <n-icon size="20">
                <PhGearSix />
              </n-icon>
            </template>
            配置
          </n-button>
          <n-button size="small" @click="aboutDialogShowing = true">
            <template #icon>
              <n-icon size="20">
                <PhInfo />
              </n-icon>
            </template>
            关于
          </n-button>
        </div>
        <ProgressesPane />
      </div>

      <LoginDialog v-model:showing="loginDialogShowing" />
      <LogDialog v-model:showing="logDialogShowing" />
      <SettingsDialog v-model:showing="settingsDialogShowing" />
      <AboutDialog v-model:showing="aboutDialogShowing" />
    </div>
  </div>
</template>

<style scoped>
:global(.n-notification-main__header) {
  @apply break-words;
}

:global(.n-tabs-pane-wrapper) {
  @apply h-full;
}

:deep(.n-tabs-nav) {
  @apply px-2;
}
</style>
