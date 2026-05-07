<script setup lang="ts">
import { commands } from '../bindings.ts'
import { path } from '@tauri-apps/api'
import { appDataDir } from '@tauri-apps/api/path'
import { useStore } from '../store.ts'
import {
  NButton,
  NDialog,
  NInput,
  NInputGroup,
  NInputGroupLabel,
  NInputNumber,
  NModal,
  NRadioButton,
  NRadioGroup,
  useMessage,
} from 'naive-ui'
import { ref } from 'vue'

const store = useStore()
const message = useMessage()

const showing = defineModel<boolean>('showing', { required: true })

const proxyHost = ref<string>(store.config?.proxyHost ?? '')

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
      <div class="flex flex-col">
        <span class="font-bold">下载速度</span>
        <div class="flex flex-col gap-1">
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
            <n-input-group-label size="small">更新库存时，每处理完一个已下载的漫画后休息</n-input-group-label>
            <n-input-number
              placeholder=""
              class="w-full"
              size="small"
              v-model:value="store.config.updateGetComicIntervalSec"
              :min="0"
              :parse="(x: string) => Number(x)" />
            <n-input-group-label size="small">秒</n-input-group-label>
          </n-input-group>
        </div>

        <span class="font-bold mt-2">代理类型</span>
        <n-radio-group v-model:value="store.config.proxyMode" size="small">
          <n-radio-button value="System">系统代理</n-radio-button>
          <n-radio-button value="NoProxy">直连</n-radio-button>
          <n-radio-button value="Custom">自定义</n-radio-button>
        </n-radio-group>
        <n-input-group v-if="store.config.proxyMode === 'Custom'" class="mt-1">
          <n-input-group-label size="small">http://</n-input-group-label>
          <n-input
            v-model:value="proxyHost"
            size="small"
            placeholder=""
            @blur="store.config.proxyHost = proxyHost"
            @keydown.enter="store.config.proxyHost = proxyHost" />
          <n-input-group-label size="small">:</n-input-group-label>
          <n-input-number
            v-model:value="store.config.proxyPort"
            size="small"
            placeholder=""
            :parse="(x: string) => parseInt(x)" />
        </n-input-group>

        <n-button class="ml-auto mt-2" size="small" @click="showConfigPathInFileManager">打开配置目录</n-button>
      </div>
    </n-dialog>
  </n-modal>
</template>
