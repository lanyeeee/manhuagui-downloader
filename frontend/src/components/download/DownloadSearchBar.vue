<script setup lang="ts">
import {defineModel, defineProps, ref} from "vue";
import {TreeOption, useNotification} from "naive-ui";
import {useDownloaderStore} from "../../stores/downloader";
import {ArrowUpOutline as ArrowUpIcon, SearchOutline as SearchIcon} from "@vicons/ionicons5";
import {SearchComicById, SearchComicByKeyword} from "../../../wailsjs/go/api/DownloadApi";
import {search, types} from "../../../wailsjs/go/models";
import {DownloadStatus} from "../../constants/download-constant";
import ComicSearchResult = search.ComicSearchResult;

const store = useDownloaderStore();
const notification = useNotification();

const downloadTreeOptions = defineModel<TreeOption[]>("downloadTreeOptions", {required: true});
const downloadDefaultExpandKeys = defineModel<string[]>("downloadDefaultExpandKeys", {required: true});
const downloadDefaultCheckedKeys = defineModel<string[]>("downloadDefaultCheckedKeys", {required: true});
const searchResultType = defineModel<"empty" | "tree" | "list">("searchResultType", {required: true});

const searchByKeywordInput = ref<string>("");
const searchByKeywordButtonLoading = ref<boolean>(false);
const searchByKeywordButtonDisabled = ref<boolean>(false);

const searchByIdInput = ref<string>("");
const searchByIdButtonLoading = ref<boolean>(false);
const searchByIdButtonDisabled = ref<boolean>(false);

const props = defineProps<{
  disabled: boolean
}>();

const searchByKeywordResult = defineModel<ComicSearchResult>("searchByKeywordResult", {required: true});

function buildOptionTree(node: types.TreeNode): TreeOption {
  const nodeOption: TreeOption = {
    key: node.key,
    label: node.label,
    isLeaf: node.isLeaf,
    disabled: node.disabled,
    children: []
  };

  if (node.defaultChecked) {
    downloadDefaultCheckedKeys.value?.push(node.key);
    nodeOption.suffix = () => DownloadStatus.COMPLETED;
  }
  if (node.defaultExpand) {
    downloadDefaultExpandKeys.value?.push(node.key);
  }

  for (const child of node.children) {
    const childOption = buildOptionTree(child);
    nodeOption.children?.push(childOption);
  }

  return nodeOption;
}

async function searchByKeyword(keyword: string, pageNumber: number = 1) {
  if (props.disabled || searchByKeywordButtonDisabled.value) {
    return;
  }

  try {
    searchByKeywordButtonLoading.value = true;
    searchByIdButtonDisabled.value = true;
    const response = await SearchComicByKeyword(keyword, pageNumber, store.proxyUrl);
    if (response.code != 0) {
      notification.create({type: "error", title: "搜索失败", description: response.msg,});
      return;
    }

    const searchResult: ComicSearchResult = response.data ?? [];
    console.log("搜索结果", searchResult);
    searchByKeywordResult.value = searchResult;
    searchResultType.value = "list";
  } finally {
    searchByKeywordButtonLoading.value = false;
    searchByIdButtonDisabled.value = false;
  }

}

async function searchById(input: string) {
  if (props.disabled || searchByIdButtonDisabled.value) {
    return;
  }
  const comicId = isNumeric(input) ? input : extractComicIdFrom(input);
  if (!comicId) {
    notification.create({type: "error", title: "搜索失败", description: "请输入漫画ID或漫画链接", duration: 2000,});
    return;
  }

  try {
    searchByIdButtonLoading.value = true;
    searchByKeywordButtonDisabled.value = true;
    const response = await SearchComicById(comicId, store.proxyUrl, store.cacheDirectory);
    if (response.code != 0) {
      notification.create({type: "error", title: "搜索失败", description: response.msg,});
      return;
    }

    const root: types.TreeNode = response.data;
    console.log("搜索结果", root);
    const rootOption = buildOptionTree(root);

    downloadTreeOptions.value = [rootOption];
    searchResultType.value = "tree";

  } finally {
    searchByIdButtonLoading.value = false;
    searchByKeywordButtonDisabled.value = false;
  }
}

function isNumeric(value: string) {
  return !isNaN(Number(value));
}

function extractComicIdFrom(input: string): string | null {
  if (isNumeric(input)) {
    return input;
  }

  const regex = /\/comic\/(\d+)\//;
  const match = input.match(regex);
  if (match && match[1]) {
    return match[1];
  }
  return null;
}

defineExpose({
  searchById,
  searchByKeyword,
  searchByKeywordInput
});

</script>

<template>
  <div class="flex flex-col gap-y-2">
    <div class="flex-1 flex gap-x-2">
      <n-input class="text-align-left"
               v-model:value="searchByKeywordInput"
               placeholder=""
               clearable
               @keydown.enter="searchByKeyword(searchByKeywordInput.trim())"
      >
        <template #prefix>
          漫画名：
        </template>
      </n-input>
      <n-button @click="searchByKeyword(searchByKeywordInput.trim())"
                type="primary"
                :loading="searchByKeywordButtonLoading"
                :disabled="disabled || searchByKeywordButtonDisabled"
                secondary
      >搜索
        <template #icon>
          <n-icon>
            <search-icon/>
          </n-icon>
        </template>
      </n-button>
    </div>

    <div class="flex-1 flex gap-x-2">
      <n-input class="text-align-left"
               v-model:value="searchByIdInput"
               placeholder="链接也行"
               clearable
               @keydown.enter="searchById(searchByIdInput.trim())"
      >
        <template #prefix>
          漫画ID：
        </template>
      </n-input>

      <n-button @click="searchById(searchByIdInput.trim())"
                type="primary"
                :loading="searchByIdButtonLoading"
                :disabled="disabled || searchByIdButtonDisabled"
                secondary
      >直达
        <template #icon>
          <n-icon>
            <arrow-up-icon/>
          </n-icon>
        </template>
      </n-button>
    </div>
  </div>
</template>
