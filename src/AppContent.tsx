import { useEffect, useState } from 'react'
import { Comic, commands, Config, UserProfile } from './bindings.ts'
import { App as AntdApp, Avatar, Button, Input, Tabs, TabsProps } from 'antd'
import LoginDialog from './components/LoginDialog.tsx'
import DownloadingPane from './panes/DownloadingPane.tsx'
import { CurrentTabName } from './types.ts'
import SearchPane from './panes/SearchPane.tsx'
import ChapterPane from './panes/ChapterPane.tsx'
import { path } from '@tauri-apps/api'
import { appDataDir } from '@tauri-apps/api/path'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import FavoritePane from './panes/FavoritePane.tsx'
import DownloadedPane from './panes/DownloadedPane.tsx'

interface Props {
  config: Config
  setConfig: (value: Config | undefined | ((prev: Config | undefined) => Config | undefined)) => void
}

function AppContent({ config, setConfig }: Props) {
  const { message, notification } = AntdApp.useApp()

  const [userProfile, setUserProfile] = useState<UserProfile>()
  const [loginDialogShowing, setLoginDialogShowing] = useState<boolean>(false)
  const [pickedComic, setPickedComic] = useState<Comic>()

  useEffect(() => {
    if (config === undefined) {
      return
    }

    commands.saveConfig(config).then(async () => {
      message.success('保存配置成功')
    })
  }, [config, message])

  useEffect(() => {
    if (config.cookie === '') {
      return
    }

    commands.getUserProfile().then(async (result) => {
      if (result.status === 'error') {
        notification.error({
          message: '获取用户信息失败',
          description: result.error,
          duration: 0,
        })
        setUserProfile(undefined)
        return
      }

      setUserProfile(result.data)
      message.success('获取用户信息成功')
    })
  }, [config.cookie, message, notification])

  async function revealConfigPath() {
    try {
      const configPath = await path.join(await appDataDir(), 'config.json')
      await revealItemInDir(configPath)
    } catch (error) {
      if (typeof error === 'string') {
        notification.error({
          message: '打开配置目录失败',
          description: `打开配置目录失败: ${error}`,
          duration: 0,
        })
      } else {
        notification.error({
          message: '打开配置目录失败',
          description: `打开配置目录失败，请联系开发者`,
          duration: 0,
        })
        console.error(error)
      }
    }
  }

  async function test() {
    const result = await commands.updateDownloadedComics()
    console.log(result)
  }

  const [currentTabName, setCurrentTabName] = useState<CurrentTabName>('search')

  const tabItems: TabsProps['items'] = [
    {
      key: 'search',
      label: '漫画搜索',
      children: <SearchPane setPickedComic={setPickedComic} setCurrentTabName={setCurrentTabName} />,
    },
    {
      key: 'favorite',
      label: '漫画收藏',
      children: (
        <FavoritePane userProfile={userProfile} setPickedComic={setPickedComic} setCurrentTabName={setCurrentTabName} />
      ),
    },
    {
      key: 'downloaded',
      label: '本地库存',
      children: (
        <DownloadedPane
          config={config}
          setConfig={setConfig}
          setPickedComic={setPickedComic}
          currentTabName={currentTabName}
          setCurrentTabName={setCurrentTabName}
        />
      ),
    },
    {
      key: 'chapter',
      label: '章节详情',
      children: <ChapterPane pickedComic={pickedComic} setPickedComic={setPickedComic} />,
    },
  ]

  return (
    <div className="h-screen flex flex-col">
      <div className="flex">
        <Input
          prefix="Cookie："
          value={config.cookie}
          onChange={(e) => setConfig({ ...config, cookie: e.target.value })}
          allowClear={true}
        />
        <Button type="primary" onClick={() => setLoginDialogShowing(true)}>
          账号登录
        </Button>
        <Button onClick={revealConfigPath}>打开配置目录</Button>
        <Button onClick={test}>测试用</Button>
        {userProfile !== undefined && (
          <div className="flex items-center">
            <Avatar src={userProfile.avatar} />
            <span className="whitespace-nowrap">{userProfile.username}</span>
          </div>
        )}
      </div>
      <div className="flex flex-1 overflow-hidden">
        <Tabs
          size="small"
          items={tabItems}
          className="h-full basis-1/2"
          activeKey={currentTabName}
          onChange={(key) => setCurrentTabName(key as CurrentTabName)}
        />
        <DownloadingPane className="h-full basis-1/2 overflow-auto" config={config} setConfig={setConfig} />
      </div>

      <LoginDialog
        loginDialogShowing={loginDialogShowing}
        setLoginDialogShowing={setLoginDialogShowing}
        config={config}
        setConfig={setConfig}
      />
    </div>
  )
}

export default AppContent
