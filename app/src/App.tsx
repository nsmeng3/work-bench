import { useState } from "react";
import { AppShell } from "./components/AppShell";
import type { NavItem } from "./components/AppShell";
import { SpacePage } from "./pages/SpacePage";
import "./styles/theme.css";

const navItems: NavItem[] = [
  { key: "spaces", label: "空间", icon: "◆", enabled: true },
  { key: "collections", label: "资源集", icon: "◇", enabled: false },
  { key: "inbox", label: "收件箱", icon: "◇", enabled: false },
  { key: "settings", label: "设置", icon: "◇", enabled: false },
];

function App() {
  const [activeNav, setActiveNav] = useState("spaces");

  return (
    <AppShell navItems={navItems} activeNav={activeNav} onNavChange={setActiveNav}>
      {activeNav === "spaces" && <SpacePage />}
    </AppShell>
  );
}

export default App;
