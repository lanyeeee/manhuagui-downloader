import { Config } from '../bindings.ts'
import { App as AntdApp, Button, InputNumber, Modal } from 'antd'
import { path } from '@tauri-apps/api'
import { appDataDir } from '@tauri-apps/api/path'
import { revealItemInDir } from '@tauri-apps/plugin-opener'

interface Props {
  settingsDialogShowing: boolean
  setSettingsDialogShowing: (showing: boolean) => void
  config: Config
  setConfig: (value: Config | undefined | ((prev: Config | undefined) => Config | undefined)) => void
}

function SettingsDialog({ settingsDialogShowing, setSettingsDialogShowing, config, setConfig }: Props) {
  const { notification } = AntdApp.useApp()

  async function revealConfigPath() {
    const configPath = await path.join(await appDataDir(), 'config.json')
    try {
      await revealItemInDir(configPath)
    } catch (error) {
      if (typeof error === 'string') {
        notification.error({
          message: '打开配置目录失败',
          description: `打开配置目录"${configPath}失败: ${error}`,
          duration: 0,
        })
      } else {
        notification.error({
          message: '打开配置目录失败',
          description: `打开配置目录"${configPath}失败，请联系开发者`,
          duration: 0,
        })
        console.error(error)
      }
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
          <Button size="small" onClick={revealConfigPath}>
            打开配置目录
          </Button>
        </div>
      </div>
    </Modal>
  )
}

export default SettingsDialog
