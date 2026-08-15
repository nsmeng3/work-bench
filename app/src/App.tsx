import { useState } from "react";
import { ConfigProvider, App as AntApp } from "antd";
import zhCN from "antd/locale/zh_CN";
import {
  AppstoreOutlined,
  FilterOutlined,
  InboxOutlined,
  SettingOutlined,
  FolderOutlined,
} from "@ant-design/icons";
import { AppShell } from "./components/AppShell";
import type { NavItem } from "./components/AppShell";
import { SpacePage } from "./pages/SpacePage";
import { CollectionPage } from "./pages/CollectionPage";
import { CollectionDetailPage } from "./pages/CollectionDetailPage";
import { FilterPage } from "./pages/FilterPage";
import type { Collection, Space } from "./api";
import "./styles/theme.css";

const navItems: NavItem[] = [
  { key: "spaces", label: "空间", icon: <AppstoreOutlined />, enabled: true },
  { key: "filter", label: "筛选", icon: <FilterOutlined />, enabled: true },
  { key: "collections", label: "资源集", icon: <FolderOutlined />, enabled: false },
  { key: "inbox", label: "收件箱", icon: <InboxOutlined />, enabled: false },
  { key: "settings", label: "设置", icon: <SettingOutlined />, enabled: false },
];

function App() {
  const [activeNav, setActiveNav] = useState("spaces");
  /** 当前下钻进入的空间；null 表示在空间列表页 */
  const [currentSpace, setCurrentSpace] = useState<Space | null>(null);
  /** 当前下钻进入的资源集；null 表示在资源集列表页 */
  const [currentCollection, setCurrentCollection] = useState<Collection | null>(null);

  function handleNavChange(key: string) {
    setActiveNav(key);
    // 切换主导航时退出下钻
    setCurrentSpace(null);
    setCurrentCollection(null);
  }

  function handleBackToSpaces() {
    setCurrentSpace(null);
    setCurrentCollection(null);
  }

  function handleEnterSpace(space: Space) {
    setCurrentSpace(space);
    setCurrentCollection(null);
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
            (currentSpace && currentCollection ? (
              <CollectionDetailPage
                space={currentSpace}
                collection={currentCollection}
                onBack={() => setCurrentCollection(null)}
              />
            ) : currentSpace ? (
              <CollectionPage
                space={currentSpace}
                onBack={handleBackToSpaces}
                onEnterCollection={setCurrentCollection}
              />
            ) : (
              <SpacePage onEnterSpace={handleEnterSpace} />
            ))}
          {activeNav === "filter" && <FilterPage />}
        </AppShell>
      </AntApp>
    </ConfigProvider>
  );
}

export default App;
