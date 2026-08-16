import { useEffect, useState } from "react";
import { ConfigProvider, App as AntApp, Spin } from "antd";
import zhCN from "antd/locale/zh_CN";
import {
  AppstoreOutlined,
  AuditOutlined,
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
import { InitWizardPage } from "./pages/InitWizardPage";
import { AuditPage } from "./pages/AuditPage";
import { settingsGetRootDir, toApiError } from "./api";
import type { Collection, Space } from "./api";
import "./styles/theme.css";

const navItems: NavItem[] = [
  { key: "spaces", label: "空间", icon: <AppstoreOutlined />, enabled: true },
  { key: "filter", label: "筛选", icon: <FilterOutlined />, enabled: true },
  { key: "audit", label: "审计", icon: <AuditOutlined />, enabled: true },
  { key: "collections", label: "资源集", icon: <FolderOutlined />, enabled: false },
  { key: "inbox", label: "收件箱", icon: <InboxOutlined />, enabled: false },
  { key: "settings", label: "设置", icon: <SettingOutlined />, enabled: false },
];

/** 首启检测状态：loading → ready / failed */
type BootstrapPhase =
  | { kind: "loading" }
  | { kind: "ready"; initialized: boolean }
  | { kind: "failed"; message: string };

function App() {
  const [phase, setPhase] = useState<BootstrapPhase>({ kind: "loading" });
  const [activeNav, setActiveNav] = useState("spaces");
  /** 当前下钻进入的空间；null 表示在空间列表页 */
  const [currentSpace, setCurrentSpace] = useState<Space | null>(null);
  /** 当前下钻进入的资源集；null 表示在资源集列表页 */
  const [currentCollection, setCurrentCollection] = useState<Collection | null>(null);

  /**
   * 首启检测：调 settings_get_root_dir。
   * initialized=false 时强制渲染初始化向导（等效于路由 /init），
   * 主界面其它入口隐藏；初始化成功后切回主界面。
   */
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const status = await settingsGetRootDir();
        if (cancelled) return;
        setPhase({ kind: "ready", initialized: status.initialized });
      } catch (err) {
        if (cancelled) return;
        const apiErr = toApiError(err);
        setPhase({ kind: "failed", message: apiErr.message });
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

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

  function handleInitialized(_rootDir: string) {
    // 初始化成功后切回主界面（等效于已初始化状态）
    setPhase({ kind: "ready", initialized: true });
    setActiveNav("spaces");
    setCurrentSpace(null);
    setCurrentCollection(null);
  }

  /**
   * 筛选页「跳转到资源集」（m5-filter-jump）：
   * 切到 spaces 主导航并下钻到指定 space/collection。
   */
  function handleJumpToCollection(space: Space, collection: Collection) {
    setActiveNav("spaces");
    setCurrentSpace(space);
    setCurrentCollection(collection);
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
        {phase.kind === "loading" && (
          <div
            style={{
              minHeight: "100vh",
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
            }}
          >
            <Spin size="large" tip="正在加载配置…" />
          </div>
        )}

        {phase.kind === "failed" && (
          <div
            style={{
              minHeight: "100vh",
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              padding: 24,
            }}
          >
            <div style={{ maxWidth: 480, textAlign: "center" }}>
              <h2>配置加载失败</h2>
              <p style={{ color: "#666" }}>{phase.message}</p>
            </div>
          </div>
        )}

        {phase.kind === "ready" && !phase.initialized && (
          <InitWizardPage onInitialized={handleInitialized} />
        )}

        {phase.kind === "ready" && phase.initialized && (
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
            {activeNav === "filter" && <FilterPage onJumpToCollection={handleJumpToCollection} />}
            {activeNav === "audit" && <AuditPage />}
          </AppShell>
        )}
      </AntApp>
    </ConfigProvider>
  );
}

export default App;
