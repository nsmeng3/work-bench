import type { ReactNode } from "react";
import { Layout, Menu } from "antd";
import type { MenuProps } from "antd";
import { ErrorBoundary } from "./ErrorBoundary";

export interface NavItem {
  key: string;
  label: string;
  icon?: ReactNode;
  enabled: boolean;
}

interface AppShellProps {
  navItems: NavItem[];
  activeNav: string;
  onNavChange: (key: string) => void;
  children: ReactNode;
}

export function AppShell({ navItems, activeNav, onNavChange, children }: AppShellProps) {
  const menuItems: MenuProps["items"] = navItems.map((item) => ({
    key: item.key,
    label: item.label,
    icon: item.icon,
    disabled: !item.enabled,
  }));

  return (
    <Layout style={{ height: "100vh" }}>
      <Layout.Sider width={200} theme="dark">
        <div
          style={{
            color: "#fff",
            fontSize: 16,
            fontWeight: 600,
            padding: "16px",
            borderBottom: "1px solid rgba(255,255,255,0.1)",
            marginBottom: 8,
          }}
        >
          资源管理工作台
        </div>
        <Menu
          theme="dark"
          mode="inline"
          selectedKeys={[activeNav]}
          items={menuItems}
          onClick={({ key }) => onNavChange(key)}
        />
      </Layout.Sider>
      <Layout>
        <Layout.Content style={{ padding: 24, overflowY: "auto" }}>
          {/* 页面级错误边界：单页崩溃不白屏整个应用，侧边栏导航保持可用 */}
          <ErrorBoundary>{children}</ErrorBoundary>
        </Layout.Content>
      </Layout>
    </Layout>
  );
}
