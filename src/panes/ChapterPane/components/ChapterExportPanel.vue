<script setup lang="tsx">
import { PartialSelectionOptions, SelectionArea, SelectionEvent } from '@viselect/vue'
import { ChapterInfo, commands, DownloadTaskState } from '../../../bindings.ts'
import {
  DropdownOption,
  NButton,
  NCheckbox,
  NDropdown,
  NIcon,
  NPopover,
  NRadioButton,
  NRadioGroup,
  NTabPane,
  NTabs,
} from 'naive-ui'
import { useStore } from '../../../store.ts'
import { ChapterPaneMode } from '../ChapterPane.vue'
import { PhPalette } from '@phosphor-icons/vue'
import { computed, defineComponent, nextTick, PropType, ref, useTemplateRef, watch, watchEffect } from 'vue'

type State = DownloadTaskState | 'Idle'

const store = useStore()

const props = defineProps<{
  sortedGroups: [string, ChapterInfo[]][]
  reload: () => void
}>()

const chapterPaneMode = defineModel<ChapterPaneMode>('chapterPaneMode', { required: true })
const currentGroupName = defineModel<string>('currentGroupName', { required: true })
const currentGroup = computed<ChapterInfo[] | undefined>(
  () => props.sortedGroups.find(([groupName]) => groupName === currentGroupName.value)?.[1],
)

const selectionOptions: PartialSelectionOptions = {
  selectables: '.selectable',
  features: { deselectOnBlur: true },
  boundaries: '.chapter-export-pane-selection-container',
}
const selectionAreaRef = useTemplateRef('selectionAreaRef')
const checkedIds = ref<Set<number>>(new Set())
const selectedIds = ref<Set<number>>(new Set())
const exportingChapterIds = ref<Set<number>>(new Set())

function clearCheckedAndSelected() {
  checkedIds.value.clear()
  selectedIds.value.clear()
  selectionAreaRef.value?.selection?.clearSelection()
}

watch(
  () => store.pickedComic,
  () => {
    clearCheckedAndSelected()
    exportingChapterIds.value.clear()
  },
)

watchEffect(() => {
  if (store.pickedComic === undefined) {
    return
  }

  const selectableChapterIds = new Set(
    props.sortedGroups
      .flatMap(([, chapters]) => chapters)
      .filter((chapter) => isChapterSelectable(chapter))
      .map((chapter) => chapter.chapterId),
  )

  for (const id of checkedIds.value) {
    if (!selectableChapterIds.has(id)) {
      checkedIds.value.delete(id)
    }
  }

  for (const id of selectedIds.value) {
    if (!selectableChapterIds.has(id)) {
      selectedIds.value.delete(id)
    }
  }
})

function extractIds(elements: Element[]): number[] {
  return elements
    .map((element) => element.getAttribute('data-key'))
    .filter(Boolean)
    .map(Number)
    .filter((id) => currentGroup.value?.some((chapter) => chapter.chapterId === id) ?? false)
}

function unselectAll({ event, selection }: SelectionEvent) {
  if (!event?.ctrlKey && !event?.metaKey) {
    selection.clearSelection()
    selectedIds.value.clear()
  }
}

function updateSelectedIds({
  store: {
    changed: { added, removed },
  },
}: SelectionEvent) {
  extractIds(added).forEach((id) => selectedIds.value.add(id))
  extractIds(removed).forEach((id) => selectedIds.value.delete(id))
}

const dropdownX = ref<number>(0)
const dropdownY = ref<number>(0)
const dropdownShowing = ref<boolean>(false)
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
    key: 'check-all',
    props: {
      onClick: () => {
        currentGroup.value?.filter((chapter) => isChapterSelectable(chapter)).forEach((chapter) => {
          checkedIds.value.add(chapter.chapterId)
        })
        dropdownShowing.value = false
      },
    },
  },
  {
    label: '取消全选',
    key: 'uncheck-all',
    props: {
      onClick: () => {
        currentGroup.value?.forEach((chapter) => checkedIds.value.delete(chapter.chapterId))
        dropdownShowing.value = false
      },
    },
  },
]

async function showDropdown(e: MouseEvent) {
  dropdownShowing.value = false
  await nextTick()
  dropdownShowing.value = true
  dropdownX.value = e.clientX
  dropdownY.value = e.clientY
}

async function exportCbz() {
  if (store.pickedComic === undefined) {
    return
  }

  const chapterIds = currentGroup.value
    ?.filter((chapter) => isChapterSelectable(chapter) && checkedIds.value.has(chapter.chapterId))
    .map((chapter) => chapter.chapterId)
  if (chapterIds === undefined || chapterIds.length === 0) {
    return
  }

  store.progressesPaneTabName = 'export'
  chapterIds.forEach((id) => exportingChapterIds.value.add(id))

  const result = await commands.exportCbzChapters(store.pickedComic, chapterIds)
  if (result.status === 'error') {
    console.error(result.error)
    exportingChapterIds.value.clear()
    return
  }

  clearCheckedAndSelected()
  exportingChapterIds.value.clear()
}

async function exportPdf() {
  if (store.pickedComic === undefined) {
    return
  }

  const chapterIds = currentGroup.value
    ?.filter((chapter) => isChapterSelectable(chapter) && checkedIds.value.has(chapter.chapterId))
    .map((chapter) => chapter.chapterId)
  if (chapterIds === undefined || chapterIds.length === 0) {
    return
  }

  store.progressesPaneTabName = 'export'
  chapterIds.forEach((id) => exportingChapterIds.value.add(id))

  const result = await commands.exportPdfChapters(store.pickedComic, chapterIds)
  if (result.status === 'error') {
    console.error(result.error)
    exportingChapterIds.value.clear()
    return
  }

  clearCheckedAndSelected()
  exportingChapterIds.value.clear()
}

function getChapterState(chapter: ChapterInfo): State {
  return store.progresses.get(chapter.chapterId)?.state ?? 'Idle'
}

function isDownloadingChapter(chapter: ChapterInfo) {
  const state = getChapterState(chapter)
  return state === 'Pending' || state === 'Downloading' || state === 'Paused'
}

function isDownloadedChapter(chapter: ChapterInfo): boolean {
  return chapter.isDownloaded === true
}

function isExportingChapter(chapter: ChapterInfo): boolean {
  return exportingChapterIds.value.has(chapter.chapterId)
}

function isChapterSelectable(chapter: ChapterInfo): boolean {
  return !isDownloadingChapter(chapter) && isDownloadedChapter(chapter) && !isExportingChapter(chapter)
}

const ChapterCheckbox = defineComponent({
  name: 'ChapterCheckbox',
  props: {
    chapter: {
      type: Object as PropType<ChapterInfo>,
      required: true,
    },
  },
  setup: (props) => {
    return () => (
      <NCheckbox
        class={[
          'hover:bg-gray-200!',
          {
            selectable: isChapterSelectable(props.chapter),
            selected: selectedIds.value.has(props.chapter.chapterId),
            downloading: isDownloadingChapter(props.chapter),
            pdfExported: props.chapter.isPdfExported && !props.chapter.isCbzExported,
            cbzExported: props.chapter.isCbzExported && !props.chapter.isPdfExported,
            exportedBoth: props.chapter.isPdfExported && props.chapter.isCbzExported,
          },
        ]}
        checked={checkedIds.value.has(props.chapter.chapterId)}
        onUpdate:checked={(checked: boolean) => {
          if (checked) {
            checkedIds.value.add(props.chapter.chapterId)
          } else {
            checkedIds.value.delete(props.chapter.chapterId)
          }
        }}
        label={props.chapter.chapterTitle}
        disabled={!isChapterSelectable(props.chapter)}
      />
    )
  },
})
</script>

<template>
  <div v-if="store.pickedComic !== undefined" class="flex-1 flex flex-col overflow-auto">
    <div class="flex items-center select-none pt-2 gap-1 px-2">
      <n-radio-group v-model:value="chapterPaneMode" size="small">
        <n-radio-button value="download">下载</n-radio-button>
        <n-radio-button value="export">导出</n-radio-button>
      </n-radio-group>
      <n-popover placement="bottom" trigger="hover" raw>
        <template #trigger>
          <n-icon class="ml-1 cursor-help" size="22"><PhPalette /></n-icon>
        </template>
        <div class="flex flex-col gap-1 text-xs leading-5 bg-white p-2 rounded-lg">
          <div class="flex items-center gap-2">
            <span class="h-3.5 w-3.5 shrink-0 rounded border border-solid border-orange bg-orange-1" />
            <span>仅 曾导出过PDF</span>
          </div>
          <div class="flex items-center gap-2">
            <span class="h-3.5 w-3.5 shrink-0 rounded border border-solid border-fuchsia bg-fuchsia-1" />
            <span>仅 曾导出过CBZ</span>
          </div>
          <div class="flex items-center gap-2">
            <span class="h-3.5 w-3.5 shrink-0 rounded border border-solid border-indigo bg-indigo-2" />
            <span>曾导出过PDF+CBZ</span>
          </div>
        </div>
      </n-popover>
      <n-button class="ml-auto" size="small" @click="props.reload">刷新</n-button>
      <n-button size="small" type="primary" @click="exportCbz">导出cbz</n-button>
      <n-button size="small" type="primary" @click="exportPdf">导出pdf</n-button>
    </div>

    <SelectionArea ref="selectionAreaRef" :options="selectionOptions" @move="updateSelectedIds" @start="unselectAll" />

    <n-tabs class="flex-1 overflow-auto" v-model:value="currentGroupName" type="line" size="small" animated>
      <n-tab-pane
        v-for="[groupName, chapters] in sortedGroups"
        :key="groupName"
        :name="groupName"
        :tab="groupName"
        class="overflow-auto p-0! h-full">
        <div
          class="chapter-export-pane-selection-container box-border p-2 overflow-auto h-full"
          @contextmenu="showDropdown">
          <div class="grid grid-cols-3 gap-1.5 w-full">
            <ChapterCheckbox
              v-for="chapter in chapters"
              :key="chapter.chapterId"
              :data-key="chapter.chapterId"
              :chapter="chapter" />
          </div>
        </div>
      </n-tab-pane>
    </n-tabs>

    <n-dropdown
      placement="bottom-start"
      trigger="manual"
      :x="dropdownX"
      :y="dropdownY"
      :options="dropdownOptions"
      :show="dropdownShowing"
      :on-clickoutside="() => (dropdownShowing = false)" />
  </div>
</template>

<style scoped>
.chapter-export-pane-selection-container {
  @apply select-none overflow-auto;
}

.chapter-export-pane-selection-container .pdfExported {
  @apply bg-orange-1;
}

.chapter-export-pane-selection-container .cbzExported {
  @apply bg-fuchsia-1;
}

.chapter-export-pane-selection-container .exportedBoth {
  @apply bg-indigo-2;
}

.chapter-export-pane-selection-container .selected {
  @apply bg-[rgb(204,232,255)] !important;
}

.chapter-export-pane-selection-container .downloading {
  @apply bg-[rgba(114,46,209,0.16)];
}

:deep(.n-checkbox__label) {
  @apply overflow-hidden whitespace-nowrap text-ellipsis;
}

:global(.selection-area) {
  @apply bg-[rgba(46,115,252,0.5)];
}
</style>
