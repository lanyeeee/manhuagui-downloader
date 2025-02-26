import { Modal, Typography } from 'antd'
import { useEffect, useState } from 'react'
import { getVersion } from '@tauri-apps/api/app'

interface Props {
  aboutDialogShowing: boolean
  setAboutDialogShowing: (showing: boolean) => void
}

export function AboutDialog({ aboutDialogShowing, setAboutDialogShowing }: Props) {
  const [version, setVersion] = useState<string>('')

  useEffect(() => {
    getVersion().then(setVersion)
  }, [])

  return (
    <Modal open={aboutDialogShowing} onCancel={() => setAboutDialogShowing(false)} footer={null}>
      <div className="flex flex-col items-center gap-row-6">
        <img src="../../src-tauri/icons/128x128.png" alt="logo" className="w-32 h-32" />
        <div className="text-center text-gray-400 text-xs">
          <div>
            如果本项目对你有帮助，欢迎来
            <Typography.Link href="https://github.com/lanyeeee/manhuagui-downloader" target="_blank">
              GitHub
            </Typography.Link>
            点个Star⭐支持！
          </div>
          <div className="mt-1">你的支持是我持续更新维护的动力🙏</div>
        </div>
        <div className="flex flex-col w-full gap-row-3 px-6">
          <div className="flex items-center justify-between py-2 px-4 bg-gray-100 rounded-lg">
            <span className="text-gray-500">软件版本</span>
            <div className="font-medium">v{version}</div>
          </div>
          <div className="flex items-center justify-between py-2 px-4 bg-gray-100 rounded-lg">
            <span className="text-gray-500">开源地址</span>
            <Typography.Link href="https://github.com/lanyeeee/manhuagui-downloader" target="_blank">
              GitHub
            </Typography.Link>
          </div>
          <div className="flex items-center justify-between py-2 px-4 bg-gray-100 rounded-lg">
            <span className="text-gray-500">问题反馈</span>
            <Typography.Link href="https://github.com/lanyeeee/manhuagui-downloader/issues" target="_blank">
              GitHub Issues
            </Typography.Link>
          </div>
        </div>
        <div className="flex flex-col text-xs text-gray-400">
          <div>
            Copyright © 2025{' '}
            <Typography.Link href="https://github.com/lanyeeee" target="_blank">
              lanyeeee
            </Typography.Link>
          </div>
          <div>
            Released under{' '}
            <Typography.Link href="https://github.com/lanyeeee/manhuagui-downloader/blob/main/LICENSE" target="_blank">
              MIT License
            </Typography.Link>
          </div>
        </div>
      </div>
    </Modal>
  )
}
