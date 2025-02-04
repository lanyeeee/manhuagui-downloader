import { commands, Config } from '../bindings.ts'
import { Button, InputNumber, Modal } from 'antd'
import { path } from '@tauri-apps/api'
import { appDataDir } from '@tauri-apps/api/path'

interface Props {
  settingsDialogShowing: boolean
  setSettingsDialogShowing: (showing: boolean) => void
  config: Config
  setConfig: (value: Config | undefined | ((prev: Config | undefined) => Config | undefined)) => void
}

function SettingsDialog({ settingsDialogShowing, setSettingsDialogShowing, config, setConfig }: Props) {
  async function showConfigPathInFileManager() {
    const configPath = await path.join(await appDataDir(), 'config.json')
    const result = await commands.showPathInFileManager(configPath)
    if (result.status === 'error') {
      console.error(result.error)
    }
  }

  return (
    <Modal title="更多设置" open={settingsDialogShowing} onCancel={() => setSettingsDialogShowing(false)} footer={null}>
      <div className="flex flex-col">
        <InputNumber
          size="small"
          addonBefore="每个章节下载完成后休息"
          defaultValue={config.downloadIntervalSec}
          addonAfter="秒"
          min={0}
          onChange={(value) => {
            if (value === null) {
              return
            }
            setConfig({ ...config, downloadIntervalSec: value })
          }}
        />
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
