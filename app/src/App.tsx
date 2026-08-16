import { useEffect, useState } from "react";
import { ConfigProvider, App as AntApp, Badge, Spin } from "antd";
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
import { InboxPage } from "./pages/InboxPage";
import { inboxStats, settingsGetRootDir, toApiError } from "./api";
import type { Collection, Space } from "./api";
import "./styles/theme.css";

/** 角标轮询间隔（毫秒）：30s，与 5.8 合并通知节奏对齐 */
const INBOX_BADGE_POLL_MS = 30_000;

const navItems: NavItem[] = [
  { key: "spaces", label: "空间", icon: <AppstoreOutlined />, enabled: true },
  { key: "filter", label: "筛选", icon: <FilterOutlined />, enabled: true },
  { key: "audit", label: "审计", icon: <AuditOutlined />, enabled: true },
  { key: "inbox", label: "收件箱", icon: <InboxOutlined />, enabled: true },
  { key: "collections", label: "资源集", icon: <FolderOutlined />, enabled: false },
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
  /** 收件箱待处理计数（侧边栏角标）；拉取失败时保持上次值 */
  const [inboxPending, setInboxPending] = useState(0);

  /** 侧边栏导航：收件箱项 label 包装 Badge 显示 pending 计数。
   *  NavItem.label 类型为 string，但 antd Menu 实际接受 ReactNode；
   *  此处通过类型断言传入 Badge 包装节点，避免改动 AppShell。 */
  const navItemsWithBadge: NavItem[] = navItems.map((item) =>
    item.key === "inbox"
      ? {
          ...item,
          label: (
            <Badge count={inboxPending} size="small" offset={[6, 0]}>
              {item.label}
            </Badge>
          ) as unknown as string,
        }
      : item,
  );

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

  /**
   * 收件箱角标：初始化完成后启动轮询 inbox_stats.pending。
   * 失败静默（保持上次值），避免打扰主流程。
   */
  useEffect(() => {
    if (phase.kind !== "ready" || !phase.initialized) return;
    let cancelled = false;
    const tick = async () => {
      try {
        const s = await inboxStats();
        if (!cancelled) setInboxPending(s.pending);
      } catch {
        // 静默：后端未就绪或网络异常时保持上次值
      }
    };
    void tick();
    const timer = window.setInterval(() => void tick(), INBOX_BADGE_POLL_MS);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [phase]);

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
          <AppShell navItems={navItemsWithBadge} activeNav={activeNav} onNavChange={handleNavChange}>
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
            {activeNav === "audit" && <AuditPage />}
            {activeNav === "inbox" && <InboxPage />}
          </AppShell>
        )}
      </AntApp>
    </ConfigProvider>
  );
}

export default App;
