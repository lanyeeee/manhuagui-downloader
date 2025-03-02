import { commands, Config } from '../bindings.ts'
import { App as AntdApp, Button, InputNumber, Modal } from 'antd'
import { path } from '@tauri-apps/api'
import { appDataDir } from '@tauri-apps/api/path'

interface Props {
  settingsDialogShowing: boolean
  setSettingsDialogShowing: (showing: boolean) => void
  config: Config
  setConfig: (value: Config | undefined | ((prev: Config | undefined) => Config | undefined)) => void
}

function SettingsDialog({ settingsDialogShowing, setSettingsDialogShowing, config, setConfig }: Props) {
  const { message } = AntdApp.useApp()

  async function showConfigPathInFileManager() {
    const configPath = await path.join(await appDataDir(), 'config.json')
    const result = await commands.showPathInFileManager(configPath)
    if (result.status === 'error') {
      console.error(result.error)
    }
  }

  return (
    <Modal title="更多设置" open={settingsDialogShowing} onCancel={() => setSettingsDialogShowing(false)} footer={null}>
      <div className="flex flex-col gap-row-2">
        <div className="flex gap-col-1">
          <InputNumber
            size="small"
            addonBefore="章节并发数"
            defaultValue={config.chapterConcurrency}
            min={1}
            onChange={async (value) => {
              if (value === null) {
                return
              }
              message.warning('对章节并发数的修改需要重启才能生效')
              setConfig({ ...config, chapterConcurrency: value })
            }}
          />
          <InputNumber
            size="small"
            addonBefore="每个章节下载完成后休息"
            defaultValue={config.chapterDownloadIntervalSec}
            addonAfter="秒"
            min={0}
            onChange={(value) => {
              if (value === null) {
                return
              }
              setConfig({ ...config, chapterDownloadIntervalSec: value })
            }}
          />
        </div>
        <div className="flex gap-col-1">
          <InputNumber
            size="small"
            addonBefore="图片并发数"
            defaultValue={config.imgConcurrency}
            min={1}
            onChange={async (value) => {
              if (value === null) {
                return
              }
              message.warning('对图片并发数的修改需要重启才能生效')
              setConfig({ ...config, imgConcurrency: value })
            }}
          />
          <InputNumber
            size="small"
            addonBefore="每张图片下载完成后休息"
            defaultValue={config.imgDownloadIntervalSec}
            addonAfter="秒"
            min={0}
            onChange={(value) => {
              if (value === null) {
                return
              }
              setConfig({ ...config, imgDownloadIntervalSec: value })
            }}
          />
        </div>
        <div className="flex justify-end mt-4">
          <Button size="small" onClick={showConfigPathInFileManager}>
            打开配置目录
          </Button>
        </div>
      </div>
    </Modal>
  )
}

export default SettingsDialog
