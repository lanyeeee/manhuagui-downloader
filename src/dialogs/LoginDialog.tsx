import { App as AntdApp, Input, Modal } from 'antd'
import { commands, Config } from '../bindings.ts'
import { KeyboardEvent, useState } from 'react'

interface Props {
  loginDialogShowing: boolean
  setLoginDialogShowing: (showing: boolean) => void
  config: Config
  setConfig: (value: Config | undefined | ((prev: Config | undefined) => Config | undefined)) => void
}

function LoginDialog({ loginDialogShowing, setLoginDialogShowing, config, setConfig }: Props) {
  const { message } = AntdApp.useApp()

  const [username, setUsername] = useState<string>('')
  const [password, setPassword] = useState<string>('')

  async function login() {
    if (username === '') {
      message.error('请输入用户名')
      return
    }

    if (password === '') {
      message.error('请输入密码')
      return
    }

    const key = 'login'
    message.loading({ content: '登录中...', key, duration: 0 })
    const result = await commands.login(username, password)
    message.destroy(key)
    if (result.status === 'error') {
      console.error(result.error)
      return
    }

    message.success('登录成功')
    setConfig({ ...config, cookie: result.data })
    setLoginDialogShowing(false)
  }

  async function handleKeyDown(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      await login()
    }
  }

  return (
    <Modal
      title="账号登录"
      open={loginDialogShowing}
      onOk={login}
      onCancel={() => setLoginDialogShowing(false)}
      cancelButtonProps={{ style: { display: 'none' } }}
      okText="登录">
      <div className="flex flex-col" onKeyDown={handleKeyDown}>
        <Input prefix="用户名:" value={username} onChange={(e) => setUsername(e.target.value)} />
        <Input.Password prefix="密码:" value={password} onChange={(e) => setPassword(e.target.value)} />
      </div>
    </Modal>
  )
}

export default LoginDialog
