import { Config } from '../bindings.ts'
import { InputNumber, Modal } from 'antd'

interface Props {
  settingsDialogShowing: boolean
  setSettingsDialogShowing: (showing: boolean) => void
  config: Config
  setConfig: (value: Config | undefined | ((prev: Config | undefined) => Config | undefined)) => void
}

function SettingsDialog({ settingsDialogShowing, setSettingsDialogShowing, config, setConfig }: Props) {
  return (
    <Modal
      title="更多设置"
      open={settingsDialogShowing}
      onCancel={() => setSettingsDialogShowing(false)}
      okButtonProps={{ style: { display: 'none' } }}
      cancelButtonProps={{ style: { display: 'none' } }}
      okText="登录">
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
      </div>
    </Modal>
  )
}

export default SettingsDialog
