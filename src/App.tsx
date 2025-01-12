import { useEffect, useRef, useState } from 'react'
import './App.css'
import { commands, Config, UserProfile } from './bindings.ts'
import { App as AntdApp, Avatar, Button, Input } from 'antd'
import LoginDialog from './components/LoginDialog.tsx'

function App() {
  const { message, notification } = AntdApp.useApp()

  const hasRendered = useRef(false)

  const [config, setConfig] = useState<Config>()
  // TODO: 把userProfile显示出来
  const [userProfile, setUserProfile] = useState<UserProfile>()
  const [loginDialogShowing, setLoginDialogShowing] = useState<boolean>(false)

  useEffect(() => {
    if (hasRendered.current === false || config === undefined) {
      return
    }

    commands.saveConfig(config).then(async () => {
      message.success('保存配置成功')
    })
  }, [config])

  useEffect(() => {
    if (hasRendered.current === false || config === undefined || config.cookie === '') {
      return
    }

    commands.getUserProfile().then(async (result) => {
      if (result.status === 'error') {
        notification.error({ message: '获取用户信息失败', description: result.error, duration: 0 })
        setUserProfile(undefined)
        return
      }

      setUserProfile(result.data)
      message.success('获取用户信息成功')
    })
  }, [config?.cookie])

  useEffect(() => {
    commands.getConfig().then((result) => {
      setConfig(result)
    })

    hasRendered.current = true
  }, [])

  async function test() {
    const result = await commands.getComic(20082)
    console.log(result)
  }

  return (
    <>
      {config !== undefined && (
        <div className="h-full flex flex-col">
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
            <Button onClick={test}>测试用</Button>
            {userProfile !== undefined && (
              <div className="flex items-center">
                <Avatar src={userProfile.avatar} />
                <span className="whitespace-nowrap">{userProfile.username}</span>
              </div>
            )}
          </div>

          <LoginDialog
            loginDialogShowing={loginDialogShowing}
            setLoginDialogShowing={setLoginDialogShowing}
            config={config}
            setConfig={setConfig}
          />
        </div>
      )}
    </>
  )
}

export default () => (
  <AntdApp notification={{ placement: 'bottomRight', showProgress: true }}>
    <App />
  </AntdApp>
)
