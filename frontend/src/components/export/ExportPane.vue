<script setup lang="ts">
import ExportTree from "./ExportTree.vue";
import ExportBar from "./ExportBar.vue";
import ExportRefreshButton from "./ExportRefreshButton.vue";
import CacheDirectoryInput from "../settings/CacheDirectoryInput.vue";
import {TreeInst, TreeOption} from "naive-ui";
import {computed, ref} from "vue";


const exportTreeInst = ref<TreeInst | null>(null);
const exportTreeOptions = ref<TreeOption[]>([]);
const exportDefaultExpandKeys = ref<string[]>([]);
const exportDefaultCheckedKeys = ref<string[]>([]);
const refreshDisabled = ref<boolean>(false);

const showExportTree = computed<boolean>(() => exportTreeOptions.value.length !== 0);

</script>

<template>
  <div class="flex h-full">
    <div class="flex-1 flex flex-col p-2">
      <div class="flex gap-x-2">
        <cache-directory-input/>
        <export-refresh-button :refresh-disabled="refreshDisabled"
                               v-model:export-tree-options="exportTreeOptions"
                               v-model:export-default-expand-keys="exportDefaultExpandKeys"
                               v-model:export-default-checked-keys="exportDefaultCheckedKeys"
        />
      </div>
      <n-result v-if="!showExportTree" title="缓存目录为空"/>
      <export-tree v-else
                   v-model:export-tree-inst="exportTreeInst"
                   v-model:export-tree-options="exportTreeOptions"
                   v-model:export-default-expand-keys="exportDefaultExpandKeys"
                   v-model:export-default-checked-keys="exportDefaultCheckedKeys"
      />
    </div>
    <div class="flex-1 flex flex-col p-2">
      <div class="flex-1"></div>
      <export-bar v-model:export-tree-inst="exportTreeInst"
                  v-model:export-tree-options="exportTreeOptions"
                  v-model:refresh-disabled="refreshDisabled"
      />
    </div>
  </div>
</template>
