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
  NTooltip,
  NCheckbox,
  useMessage,
} from 'naive-ui'
import { ref } from 'vue'

const store = useStore()
const message = useMessage()

const showing = defineModel<boolean>('showing', { required: true })

const proxyHost = ref<string>(store.config?.proxyHost ?? '')
const comicDirFmt = ref<string>(store.config?.comicDirFmt ?? '')
const chapterDirFmt = ref<string>(store.config?.chapterDirFmt ?? '')

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

        <span class="font-bold mt-2">导出相关</span>
        <div class="flex gap-1 items-center">
          <n-input-group class="w-70">
            <n-input-group-label size="small">创建pdf并发数</n-input-group-label>
            <n-input-number
              class="w-full"
              v-model:value="store.config.createPdfConcurrency"
              size="small"
              :min="1"
              :parse="(x: string) => Number(x)" />
          </n-input-group>
          <n-checkbox class="w-fit" v-model:checked="store.config.enableMergePdf">创建完成后是否自动合并</n-checkbox>
        </div>

        <span class="font-bold mt-2">漫画目录格式</span>
        <n-tooltip placement="top" trigger="hover">
          <div>
            可以用斜杠
            <span class="rounded bg-gray-500 px-1 select-all text-white">/</span>
            来分隔目录层级
          </div>
          <div class="font-semibold mt-2">
            <span>可用字段：</span>
          </div>
          <div>
            <span class="rounded bg-gray-500 px-1 select-all">comic_id</span>
            <span class="ml-2">漫画ID</span>
          </div>
          <div>
            <span class="rounded bg-gray-500 px-1 select-all">comic_title</span>
            <span class="ml-2">漫画标题</span>
          </div>
          <div>
            <span class="rounded bg-gray-500 px-1 select-all">comic_subtitle</span>
            <span class="ml-2">漫画副标题</span>
          </div>
          <div>
            <span class="rounded bg-gray-500 px-1 select-all">pub_year</span>
            <span class="ml-2">出版年份</span>
          </div>
          <div>
            <span class="rounded bg-gray-500 px-1 select-all">region</span>
            <span class="ml-2">地区</span>
          </div>
          <div>
            <span class="rounded bg-gray-500 px-1 select-all">author</span>
            <span class="ml-2">作者</span>
          </div>
          <div class="font-semibold mt-2">例如格式</div>
          <div class="bg-gray-200 rounded-md p-1 text-black w-fit">{author}/{comic_title}</div>
          <div class="font-semibold">
            <span>下载漫画ID为</span>
            <span class="text-blue mx-1">30252</span>
            <span>的作品的任何一个章节会创建</span>
          </div>
          <div class="flex gap-1">
            <span class="bg-gray-200 rounded-md px-1 w-fit text-black">藤本树</span>
            <span class="rounded bg-gray-500 px-1 select-all text-white">/</span>
            <span class="bg-gray-200 rounded-md px-1 w-fit text-black">电锯人</span>
          </div>
          <div class="font-semibold">
            两层文件夹，漫画元数据保存在最内层的文件夹
            <span class="bg-gray-200 rounded-md px-1 w-fit text-black font-normal">电锯人</span>
            里
          </div>
          <template #trigger>
            <n-input
              v-model:value="comicDirFmt"
              size="small"
              @blur="store.config.comicDirFmt = comicDirFmt"
              @keydown.enter="store.config.comicDirFmt = comicDirFmt" />
          </template>
        </n-tooltip>

        <span class="font-bold mt-2">章节目录格式</span>
        <n-tooltip placement="top" trigger="hover" :scrollable="true">
          <div>
            可以用斜杠
            <span class="rounded bg-gray-500 px-1 select-all text-white">/</span>
            来分隔目录层级
          </div>
          <div class="font-semibold mt-2">
            <span>可用字段：</span>
          </div>
          <div>
            <span class="rounded bg-gray-500 px-1 select-all">comic_id</span>
            <span class="ml-2">漫画ID</span>
          </div>
          <div>
            <span class="rounded bg-gray-500 px-1 select-all">comic_title</span>
            <span class="ml-2">漫画标题</span>
          </div>
          <div>
            <span class="rounded bg-gray-500 px-1 select-all">comic_subtitle</span>
            <span class="ml-2">漫画副标题</span>
          </div>
          <div>
            <span class="rounded bg-gray-500 px-1 select-all">pub_year</span>
            <span class="ml-2">出版年份</span>
          </div>
          <div>
            <span class="rounded bg-gray-500 px-1 select-all">region</span>
            <span class="ml-2">地区</span>
          </div>
          <div>
            <span class="rounded bg-gray-500 px-1 select-all">author</span>
            <span class="ml-2">作者</span>
          </div>
          <div>
            <span class="rounded bg-gray-500 px-1 select-all">group_name</span>
            <span class="ml-2">组名(单话、单行本、番外篇)</span>
          </div>
          <div>
            <span class="rounded bg-gray-500 px-1 select-all">chapter_id</span>
            <span class="ml-2">章节ID</span>
          </div>
          <div>
            <span class="rounded bg-gray-500 px-1 select-all">chapter_title</span>
            <span class="ml-2">章节标题</span>
          </div>
          <div>
            <span class="rounded bg-gray-500 px-1 select-all">order</span>
            <span class="ml-2">章节在分组中的序号，一些特殊章节会有小数点，支持补齐</span>
          </div>
          <div class="text-xs">
            <span>补齐用法：</span>
            <span class="rounded bg-gray-500 px-1 select-all font-mono">{order:0>4}</span>
            <span>表示用0补齐4位，</span>
            <span class="mr-2">例如 13 &rarr; 0013</span>
            <span>13.1 &rarr; 0013.1</span>
          </div>
          <div class="font-semibold mt-2">例如格式</div>
          <div class="bg-gray-200 rounded-md p-1 text-black w-fit">{group_name}/{order:0>3} {chapter_title}</div>
          <div class="font-semibold">
            <span>下载</span>
            <span class="text-blue mx-1">电锯人 - 单话 - 第13话</span>
            <span>会在漫画目录下再创建</span>
          </div>
          <div class="flex gap-1">
            <span class="bg-gray-200 rounded-md px-1 w-fit text-black">单话</span>
            <span class="rounded bg-gray-500 px-1 select-all text-white">/</span>
            <span class="bg-gray-200 rounded-md px-1 w-fit text-black">013 第13话</span>
          </div>
          <div class="font-semibold">
            两层文件夹，章节元数据保存在最内层的文件夹
            <span class="bg-gray-200 rounded-md px-1 w-fit text-black font-normal">013 第13话</span>
            里
          </div>
          <template #trigger>
            <n-input
              v-model:value="chapterDirFmt"
              size="small"
              @blur="store.config.chapterDirFmt = chapterDirFmt"
              @keydown.enter="store.config.chapterDirFmt = chapterDirFmt" />
          </template>
        </n-tooltip>

        <n-button class="ml-auto mt-2" size="small" @click="showConfigPathInFileManager">打开配置目录</n-button>
      </div>
    </n-dialog>
  </n-modal>
</template>
