<script setup lang="ts">
import { ChapterInfo, commands, DownloadTaskState } from '../bindings.ts'
import { PartialSelectionOptions, SelectionArea, SelectionEvent } from '@viselect/vue'
import { computed, nextTick, ref, useTemplateRef, watch, watchEffect } from 'vue'
import { useStore } from '../store.ts'
import { DropdownOption, NCheckbox, useMessage } from 'naive-ui'
import { join } from '@tauri-apps/api/path'

type State = DownloadTaskState | 'Idle'
type ChapterInfoWithState = ChapterInfo & { state: State }

const store = useStore()

const message = useMessage()

// 按章节数排序的分组
const sortedGroups = computed<[string, ChapterInfoWithState[]][] | undefined>(() => {
  if (store.pickedComic === undefined) {
    return undefined
  }

  return Object.entries(store.pickedComic.groups)
    .map(([groupPath, chapters]): [string, ChapterInfoWithState[]] => [
      groupPath,
      chapters.map((chapter) => {
        const progressData = store.progresses.get(chapter.chapterId)
        return { ...chapter, state: progressData?.state ?? 'Idle' }
      }),
    ])
    .sort((a, b) => b[1].length - a[1].length)
})
// 第一个group的名字
const firstGroupName = computed(() => sortedGroups.value?.[0]?.[0] ?? '单话')
// 当前tab的分组名
const currentGroupName = ref<string>(firstGroupName.value)
// 当前tab对应的group
const currentGroup = computed<ChapterInfoWithState[] | undefined>(() =>
  store.pickedComic?.groups[currentGroupName.value].map((chapter) => {
    const progressData = store.progresses.get(chapter.chapterId)
    return { ...chapter, state: progressData?.state ?? 'Idle' }
  }),
)
// SelectionArea的配置
const selectionOptions: PartialSelectionOptions = {
  selectables: '.selectable',
  features: { deselectOnBlur: true },
  boundaries: '.chapter-pane-selection-container',
}
// SelectionArea组件的ref
const selectionAreaRef = useTemplateRef('selectionAreaRef')
// 已勾选的章节id
const checkedIds = ref<Set<number>>(new Set())
// 用于<n-checkbox-group>
const checkedIdsArray = computed<number[]>({
  get: () => [...checkedIds.value],
  set: (next) => (checkedIds.value = new Set(next)),
})
// 已选中(被框选选到)的章节id
const selectedIds = ref<Set<number>>(new Set())
// 如果漫画变了，清空勾选和选中状态
watch(
  () => store.pickedComic,
  () => {
    checkedIds.value.clear()
    selectedIds.value.clear()
    selectionAreaRef.value?.selection?.clearSelection()
    currentGroupName.value = firstGroupName.value
  },
)
// 只保留selectable的章节
watchEffect(() => {
  if (store.pickedComic === undefined || sortedGroups.value === undefined) {
    return
  }

  const selectableChapterIds = new Set(
    sortedGroups.value
      .flatMap(([, chapters]) => chapters)
      .filter((c) => isSelectableChapter(c))
      .map((c) => c.chapterId),
  )

  checkedIds.value = new Set([...checkedIds.value].filter((id) => selectableChapterIds.has(id)))
  selectedIds.value = new Set([...selectedIds.value].filter((id) => selectableChapterIds.has(id)))
})

// 提取章节id
function extractIds(elements: Element[]): number[] {
  return elements
    .map((element) => element.getAttribute('data-key'))
    .filter(Boolean)
    .map(Number)
    .filter((id) => currentGroup.value?.find((c) => c.chapterId === id))
}
// 取消所有已选中(被框选选到)的章节
function unselectAll({ event, selection }: SelectionEvent) {
  if (!event?.ctrlKey && !event?.metaKey) {
    selection.clearSelection()
    selectedIds.value.clear()
  }
}
// 更新已选中(被框选选到)的章节id
function updateSelectedIds({
  store: {
    changed: { added, removed },
  },
}: SelectionEvent) {
  extractIds(added).forEach((id) => selectedIds.value.add(id))
  extractIds(removed).forEach((id) => selectedIds.value.delete(id))
}

// dropdown的x坐标
const dropdownX = ref<number>(0)
// dropdown的y坐标
const dropdownY = ref<number>(0)
// 是否显示dropdown
const dropdownShowing = ref<boolean>(false)
// dropdown选项
const dropdownOptions: DropdownOption[] = [
  {
    label: '勾选',
    key: 'check',
    props: {
      onClick: () => {
        selectedIds.value.forEach((id) => checkedIds.value.add(id))
        dropdownShowing.value = false
      },
    },
  },
  {
    label: '取消勾选',
    key: 'uncheck',
    props: {
      onClick: () => {
        selectedIds.value.forEach((id) => checkedIds.value.delete(id))
        dropdownShowing.value = false
      },
    },
  },
  {
    label: '全选',
    key: 'check all',
    props: {
      onClick: () => {
        currentGroup.value?.filter((c) => isSelectableChapter(c)).forEach((c) => checkedIds.value.add(c.chapterId))
        dropdownShowing.value = false
      },
    },
  },
  {
    label: '取消全选',
    key: 'uncheck all',
    props: {
      onClick: () => {
        currentGroup.value?.forEach((c) => checkedIds.value.delete(c.chapterId))

        dropdownShowing.value = false
      },
    },
  },
]
// 显示dropdown
async function showDropdown(e: MouseEvent) {
  dropdownShowing.value = false
  await nextTick()
  dropdownShowing.value = true
  dropdownX.value = e.clientX
  dropdownY.value = e.clientY
}

// 下载勾选的章节
async function downloadCheckedChapters() {
  if (store.pickedComic === undefined) {
    message.error('请先选择漫画')
    return
  }
  // 创建下载任务前，先创建元数据
  const saveMetadataResult = await commands.saveMetadata(store.pickedComic)
  if (saveMetadataResult.status === 'error') {
    console.error(saveMetadataResult.error)
    return
  }
  // 下载没有下载过的且已勾选的章节
  const chapterIdsToDownload = currentGroup.value
    ?.filter((c) => !isDownloadedChapter(c) && checkedIds.value.has(c.chapterId))
    .map((c) => c.chapterId)
  if (chapterIdsToDownload === undefined) {
    return
  }
  for (const chapterId of chapterIdsToDownload) {
    await commands.createDownloadTask(store.pickedComic, chapterId)
  }
}

// 重新加载选中的漫画
async function reloadPickedComic() {
  if (store.pickedComic === undefined) {
    return
  }

  const result = await commands.getComic(store.pickedComic.id)
  if (result.status === 'error') {
    console.error(result.error)
    return
  }

  store.pickedComic = result.data
}

async function showComicDownloadDirInFileManager() {
  if (store.config === undefined || store.pickedComic === undefined) {
    return
  }

  const comicDownloadDir = await join(store.config.downloadDir, store.pickedComic.title)
  const result = await commands.showPathInFileManager(comicDownloadDir)
  if (result.status === 'error') {
    console.error(result.error)
  }
}

function isDownloadingChapter(chapter: ChapterInfoWithState) {
  const state = chapter.state
  return state === 'Pending' || state === 'Downloading' || state === 'Paused'
}

function isDownloadedChapter(chapter: ChapterInfoWithState) {
  return chapter.isDownloaded === true
}

function isSelectableChapter(chapter: ChapterInfoWithState) {
  return !isDownloadedChapter(chapter) && !isDownloadingChapter(chapter)
}
</script>

<template>
  <div class="h-full flex flex-col box-border">
    <n-empty v-if="store.pickedComic === undefined" description="请先选择漫画(漫画搜索、漫画收藏、本地库存)" />
    <template v-else>
      <div class="flex items-center select-none pt-2 gap-1 px-2">
        <div>左键拖动进行框选，右键打开菜单</div>
        <n-button class="ml-auto" size="small" @click="reloadPickedComic">刷新</n-button>
        <n-button size="small" type="primary" @click="downloadCheckedChapters">下载勾选章节</n-button>
      </div>

      <selection-area
        ref="selectionAreaRef"
        :options="selectionOptions"
        @move="updateSelectedIds"
        @start="unselectAll" />

      <n-tabs class="flex-1 overflow-auto" v-model:value="currentGroupName" type="line" size="small" animated>
        <n-tab-pane
          v-for="[groupName, chapters] in sortedGroups"
          :key="groupName"
          :name="groupName"
          :tab="groupName"
          class="overflow-auto p-0! h-full">
          <div class="chapter-pane-selection-container box-border p-2 overflow-auto h-full" @contextmenu="showDropdown">
            <n-checkbox-group v-model:value="checkedIdsArray" class="grid grid-cols-3 gap-1.5 w-full">
              <n-checkbox
                v-for="chapter in chapters"
                :key="chapter.chapterId"
                :data-key="chapter.chapterId"
                class="hover:bg-gray-200!"
                :value="chapter.chapterId"
                :label="chapter.chapterTitle"
                :disabled="!isSelectableChapter(chapter)"
                :class="{
                  selectable: isSelectableChapter(chapter),
                  selected: selectedIds.has(chapter.chapterId),
                  downloaded: isDownloadedChapter(chapter),
                  downloading: !isDownloadedChapter(chapter) && isDownloadingChapter(chapter),
                }" />
            </n-checkbox-group>
          </div>
        </n-tab-pane>
      </n-tabs>

      <div class="flex p-2 pt-0">
        <img class="w-24 mr-4 object-cover" :src="store.pickedComic.cover" alt="" />
        <div class="flex flex-col h-full">
          <span class="font-bold text-xl line-clamp-3">
            {{ store.pickedComic.title }}
            {{ store.pickedComic.subtitle ? `(${store.pickedComic.subtitle})` : '' }}
          </span>
          <span class="text-red">作者：{{ store.pickedComic.authors.join(', ') }}</span>
          <span class="text-gray">类型：{{ store.pickedComic.genres.join(' ') }}</span>
          <n-button
            v-if="store.pickedComic.isDownloaded"
            class="flex mt-auto mr-auto"
            size="tiny"
            @click="showComicDownloadDirInFileManager">
            打开下载目录
          </n-button>
        </div>
      </div>
    </template>

    <n-dropdown
      placement="bottom-start"
      trigger="manual"
      :x="dropdownX"
      :y="dropdownY"
      :options="dropdownOptions"
      :show="dropdownShowing"
      @clickoutside="dropdownShowing = false" />
  </div>
</template>

<style scoped>
.chapter-pane-selection-container {
  @apply select-none;
}

.chapter-pane-selection-container .selected {
  @apply bg-[rgb(204,232,255)];
}

.chapter-pane-selection-container .downloaded {
  @apply bg-[rgba(24,160,88,0.16)];
}

.chapter-pane-selection-container .downloading {
  @apply bg-[rgba(114,46,209,0.16)];
}

:deep(.n-checkbox__label) {
  @apply overflow-hidden whitespace-nowrap text-ellipsis;
}

:global(.selection-area) {
  @apply bg-[rgba(46,115,252,0.5)];
}
</style>
