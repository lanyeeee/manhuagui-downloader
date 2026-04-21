<script setup lang="ts">
import { commands } from '../bindings.ts'
import { path } from '@tauri-apps/api'
import { appDataDir } from '@tauri-apps/api/path'
import { useStore } from '../store.ts'
import { useMessage } from 'naive-ui'

const store = useStore()
const message = useMessage()

const showing = defineModel<boolean>('showing', { required: true })

async function showConfigPathInFileManager() {
  const configPath = await path.join(await appDataDir(), 'config.json')
  const result = await commands.showPathInFileManager(configPath)
  if (result.status === 'error') {
    console.error(result.error)
  }
}
</script>

<template>
  <n-modal v-if="store.config !== undefined" v-model:show="showing">
    <n-dialog class="w-140!" :showIcon="false" title="设置" content-style="" @close="showing = false">
      <div class="flex flex-col gap-row-2">
        <div class="flex gap-1">
          <n-input-group class="w-35%">
            <n-input-group-label size="small">章节并发数</n-input-group-label>
            <n-input-number
              placeholder=""
              class="w-full"
              v-model:value="store.config.chapterConcurrency"
              size="small"
              @update:value="message.warning('对章节并发数的修改需要重启才能生效')"
              :min="1"
              :parse="(x: string) => Number(x)" />
          </n-input-group>
          <n-input-group class="w-65%">
            <n-input-group-label size="small">每个章节下载完成后休息</n-input-group-label>
            <n-input-number
              placeholder=""
              class="w-full"
              v-model:value="store.config.chapterDownloadIntervalSec"
              size="small"
              :min="0"
              :parse="(x: string) => Number(x)" />
            <n-input-group-label size="small">秒</n-input-group-label>
          </n-input-group>
        </div>
        <div class="flex gap-1">
          <n-input-group class="w-35%">
            <n-input-group-label size="small">图片并发数</n-input-group-label>
            <n-input-number
              placeholder=""
              class="w-full"
              v-model:value="store.config.imgConcurrency"
              size="small"
              @update-value="message.warning('对图片并发数的修改需要重启才能生效')"
              :min="1"
              :parse="(x: string) => Number(x)" />
          </n-input-group>
          <n-input-group class="w-65%">
            <n-input-group-label size="small">每张图片下载完成后休息</n-input-group-label>
            <n-input-number
              placeholder=""
              class="w-full"
              v-model:value="store.config.imgDownloadIntervalSec"
              size="small"
              :min="0"
              :parse="(x: string) => Number(x)" />
            <n-input-group-label size="small">秒</n-input-group-label>
          </n-input-group>
        </div>

        <n-input-group class="whitespace-nowrap">
          <n-input-group-label size="small">更新库存时，每获取一个已下载漫画的最新数据后休息</n-input-group-label>
          <n-input-number
            placeholder=""
            class="w-full"
            size="small"
            v-model:value="store.config.updateGetComicIntervalSec"
            :min="0"
            :parse="(x: string) => Number(x)" />
          <n-input-group-label size="small">秒</n-input-group-label>
        </n-input-group>

        <n-button class="ml-auto mt-2" size="small" @click="showConfigPathInFileManager">打开配置目录</n-button>
      </div>
    </n-dialog>
  </n-modal>
</template>
