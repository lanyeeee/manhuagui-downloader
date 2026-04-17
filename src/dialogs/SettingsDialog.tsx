import { commands, Config } from '../bindings.ts'
import { App as AntdApp, Button, InputNumber, Modal, Space } from 'antd'
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
    <Modal
      title="配置"
      open={settingsDialogShowing}
      onCancel={() => setSettingsDialogShowing(false)}
      footer={null}
      width={550}>
      <div className="flex flex-col gap-row-2">
        <div className="flex gap-col-1">
          <Space.Compact className="w-35% whitespace-nowrap">
            <Space.Addon>章节并发数</Space.Addon>
            <InputNumber
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
          </Space.Compact>

          <Space.Compact className="w-65% whitespace-nowrap">
            <Space.Addon>每个章节下载完成后休息</Space.Addon>
            <InputNumber
              className="flex-grow"
              defaultValue={config.chapterDownloadIntervalSec}
              min={0}
              onChange={(value) => {
                if (value === null) {
                  return
                }
                setConfig({ ...config, chapterDownloadIntervalSec: value })
              }}
            />
            <Space.Addon>秒</Space.Addon>
          </Space.Compact>
        </div>

        <div className="flex gap-col-1">
          <Space.Compact className="w-35% whitespace-nowrap">
            <Space.Addon>图片并发数</Space.Addon>
            <InputNumber
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
          </Space.Compact>

          <Space.Compact className="w-65% whitespace-nowrap">
            <Space.Addon>每张图片下载完成后休息</Space.Addon>
            <InputNumber
              className="flex-grow"
              defaultValue={config.imgDownloadIntervalSec}
              min={0}
              onChange={(value) => {
                if (value === null) {
                  return
                }
                setConfig({ ...config, imgDownloadIntervalSec: value })
              }}
            />
            <Space.Addon>秒</Space.Addon>
          </Space.Compact>
        </div>

        <Space.Compact className="whitespace-nowrap">
          <Space.Addon>更新库存时，每获取一个已下载漫画的最新数据后休息</Space.Addon>
          <InputNumber
            className="flex-grow"
            defaultValue={config.updateGetComicIntervalSec}
            min={0}
            onChange={(value) => {
              if (value === null) {
                return
              }
              setConfig({ ...config, updateGetComicIntervalSec: value })
            }}
          />
          <Space.Addon>秒</Space.Addon>
        </Space.Compact>

        <div className="flex justify-end mt-4">
          <Button onClick={showConfigPathInFileManager}>打开配置目录</Button>
        </div>
      </div>
    </Modal>
  )
}

export default SettingsDialog
