import { useEffect, useState } from 'react'
import './App.css'
import { commands, Config } from './bindings.ts'
import { App as AntdApp, ConfigProvider } from 'antd'
import zhCN from 'antd/es/locale/zh_CN'
import AppContent from './AppContent.tsx'

function App() {
  const [config, setConfig] = useState<Config>()
  useEffect(() => {
    // 屏蔽浏览器右键菜单
    document.oncontextmenu = (event) => {
      event.preventDefault()
    }
    // 获取配置
    commands.getConfig().then((result) => {
      setConfig(result)
    })
  }, [])

  return <>{config !== undefined && <AppContent config={config} setConfig={setConfig} />}</>
}

// eslint-disable-next-line react/display-name
export default () => (
  <AntdApp notification={{ placement: 'bottomRight', showProgress: true }}>
    <ConfigProvider locale={zhCN}>
      <App />
    </ConfigProvider>
  </AntdApp>
)
