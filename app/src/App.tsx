import { useState } from "react";
import { ConfigProvider, App as AntApp } from "antd";
import zhCN from "antd/locale/zh_CN";
import {
  AppstoreOutlined,
  InboxOutlined,
  SettingOutlined,
  FolderOutlined,
} from "@ant-design/icons";
import { AppShell } from "./components/AppShell";
import type { NavItem } from "./components/AppShell";
import { SpacePage } from "./pages/SpacePage";
import { CollectionPage } from "./pages/CollectionPage";
import type { Space } from "./api";
import "./styles/theme.css";

const navItems: NavItem[] = [
  { key: "spaces", label: "空间", icon: <AppstoreOutlined />, enabled: true },
  { key: "collections", label: "资源集", icon: <FolderOutlined />, enabled: false },
  { key: "inbox", label: "收件箱", icon: <InboxOutlined />, enabled: false },
  { key: "settings", label: "设置", icon: <SettingOutlined />, enabled: false },
];

function App() {
  const [activeNav, setActiveNav] = useState("spaces");
  /** 当前下钻进入的空间；null 表示在空间列表页 */
  const [currentSpace, setCurrentSpace] = useState<Space | null>(null);

  function handleNavChange(key: string) {
    setActiveNav(key);
    // 切换主导航时退出下钻
    setCurrentSpace(null);
  }

  return (
    <ConfigProvider
      locale={zhCN}
      theme={{
        token: {
          colorPrimary: "#4a90d9",
          colorInfo: "#4a90d9",
          borderRadius: 6,
        },
      }}
    >
      <AntApp>
        <AppShell navItems={navItems} activeNav={activeNav} onNavChange={handleNavChange}>
          {activeNav === "spaces" &&
            (currentSpace ? (
              <CollectionPage space={currentSpace} onBack={() => setCurrentSpace(null)} />
            ) : (
              <SpacePage onEnterSpace={setCurrentSpace} />
            ))}
        </AppShell>
      </AntApp>
    </ConfigProvider>
  );
}

export default App;
