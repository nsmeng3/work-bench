import { TerminalPanel, type TerminalOpenRequest } from "../components/TerminalPanel";

/** 全局终端分组的缓存键（与各空间的 groupKey 区分） */
export const GLOBAL_TERMINAL_GROUP = "__global__";

interface TerminalPageProps {
  /** 外部打开请求（如资源"在内嵌终端打开"带 cwd 跳转过来） */
  openRequest?: TerminalOpenRequest | null;
}

/**
 * m7-7.6 · 终端管理页（唯一的内嵌终端入口）。
 *
 * 承载全部终端会话：新建/切换/关闭由 TerminalPanel 的 tab 栏完成，
 * tab tooltip 显示每个会话的工作目录；
 * 资源右键"在内嵌终端打开"会带目录跳转到这里新建会话。
 */
export function TerminalPage({ openRequest }: TerminalPageProps) {
  return (
    <div style={{ display: "flex", flexDirection: "column", height: "100%" }}>
      <div style={{ marginBottom: 12 }}>
        <h1 style={{ fontSize: 20, margin: 0 }}>终端</h1>
        <span style={{ color: "#888", fontSize: 12 }}>
          全局终端会话 · 新建会话默认落在资源根目录 · 从资源右键"在内嵌终端打开"会带目录跳转到这里
        </span>
      </div>
      <div style={{ flex: 1, minHeight: 0 }}>
        <TerminalPanel groupKey={GLOBAL_TERMINAL_GROUP} openRequest={openRequest} />
      </div>
    </div>
  );
}
