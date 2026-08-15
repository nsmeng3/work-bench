import type { ReactNode } from "react";
import "./AppShell.css";

export interface NavItem {
  key: string;
  label: string;
  icon?: string;
  enabled: boolean;
}

interface AppShellProps {
  navItems: NavItem[];
  activeNav: string;
  onNavChange: (key: string) => void;
  children: ReactNode;
}

export function AppShell({ navItems, activeNav, onNavChange, children }: AppShellProps) {
  return (
    <div className="app-shell">
      <aside className="app-sidebar">
        <div className="app-sidebar-header">资源管理工作台</div>
        <nav className="app-nav">
          {navItems.map((item) => (
            <button
              key={item.key}
              className={`app-nav-item ${item.key === activeNav ? "active" : ""} ${!item.enabled ? "disabled" : ""}`}
              disabled={!item.enabled}
              onClick={() => item.enabled && onNavChange(item.key)}
            >
              {item.icon && <span className="app-nav-icon">{item.icon}</span>}
              <span className="app-nav-label">{item.label}</span>
            </button>
          ))}
        </nav>
      </aside>
      <main className="app-main">{children}</main>
    </div>
  );
}
