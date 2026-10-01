import { useCallback, useEffect, useRef, useState } from "react";
import { Button, Space as AntSpace, Tabs, Tag, message } from "antd";
import { PlusOutlined } from "@ant-design/icons";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { Channel } from "@tauri-apps/api/core";
import "@xterm/xterm/css/xterm.css";
import {
  terminalCreate,
  terminalWrite,
  terminalResize,
  terminalClose,
  type TerminalEvent,
} from "../api";

interface TerminalSession {
  /** 后端 session_id */
  sessionId: string;
  /** tab 标题（"终端 1"） */
  title: string;
  /** 是否已退出（后端推了 Exited） */
  exited: boolean;
}

interface TerminalPanelProps {
  spaceId: string;
  spaceName?: string;
}

/**
 * M7-4 · 内嵌终端面板。
 *
 * 关键设计：
 * - 进入空间详情页"终端"tab → 自动创建第一个会话。
 * - 切换 tab 不销毁会话，只切 canvas 挂载（display:none）。
 * - 离开空间（unmount）时销毁该空间所有会话。
 * - 关闭单个 tab → 调 terminal_close。
 */
export function TerminalPanel({ spaceId, spaceName }: TerminalPanelProps) {
  const [sessions, setSessions] = useState<TerminalSession[]>([]);
  const [activeKey, setActiveKey] = useState<string | undefined>(undefined);
  const [messageApi, messageContextHolder] = message.useMessage();

  /** sessionId → { term, fitAddon, containerEl } */
  const termRefs = useRef<
    Map<
      string,
      {
        term: Terminal;
        fitAddon: FitAddon;
        container: HTMLDivElement | null;
      }
    >
  >(new Map());

  const seqRef = useRef(0);
  const mountedRef = useRef(true);

  /** 创建一个新会话 */
  const createSession = useCallback(async () => {
    try {
      seqRef.current += 1;
      const title = `终端 ${seqRef.current}`;

      // 先建 xterm 实例（在 onmessage 里要用）
      const term = new Terminal({
        fontFamily: 'Menlo, Monaco, "Courier New", monospace',
        fontSize: 13,
        cursorBlink: true,
        scrollback: 5000,
        convertEol: false,
      });
      const fitAddon = new FitAddon();
      term.loadAddon(fitAddon);
      term.loadAddon(new WebLinksAddon());

      const channel = new Channel<TerminalEvent>();
      let currentSessionId = "";

      channel.onmessage = (event) => {
        if (!mountedRef.current) return;
        if (event.kind === "data") {
          term.write(event.data);
        } else if (event.kind === "exited") {
          term.write("\r\n\x1b[90m[进程已退出]\x1b[0m\r\n");
          setSessions((prev) =>
            prev.map((s) =>
              s.sessionId === currentSessionId ? { ...s, exited: true } : s,
            ),
          );
        }
      };

      // 默认 80x24，等挂载后 fitAddon.fit() 会触发 onResize 修正
      const out = await terminalCreate(
        { spaceId, cols: 80, rows: 24 },
        channel,
      );
      currentSessionId = out.sessionId;

      term.onData((data) => {
        void terminalWrite(out.sessionId, data).catch(() => void 0);
      });
      term.onResize(({ cols, rows }) => {
        void terminalResize(out.sessionId, cols, rows).catch(() => void 0);
      });

      termRefs.current.set(out.sessionId, { term, fitAddon, container: null });
      setSessions((prev) => [
        ...prev,
        { sessionId: out.sessionId, title, exited: false },
      ]);
      setActiveKey(out.sessionId);
    } catch (err) {
      messageApi.error(`创建终端失败: ${String(err)}`);
    }
  }, [spaceId, messageApi]);

  /** 关闭会话 */
  const closeSession = useCallback(
    async (sessionId: string) => {
      try {
        await terminalClose(sessionId);
      } catch {
        // 已关闭也继续走本地清理
      }
      const entry = termRefs.current.get(sessionId);
      if (entry) {
        entry.term.dispose();
        termRefs.current.delete(sessionId);
      }
      setSessions((prev) => {
        const next = prev.filter((s) => s.sessionId !== sessionId);
        setActiveKey((cur) => {
          if (cur !== sessionId) return cur;
          return next.length > 0 ? next[next.length - 1].sessionId : undefined;
        });
        return next;
      });
    },
    [],
  );

  /** 首次挂载：自动开一个会话 */
  useEffect(() => {
    mountedRef.current = true;
    void createSession();
    return () => {
      mountedRef.current = false;
      // 离开空间 → 销毁所有会话
      const ids = Array.from(termRefs.current.keys());
      for (const id of ids) {
        void terminalClose(id).catch(() => void 0);
        const entry = termRefs.current.get(id);
        if (entry) entry.term.dispose();
      }
      termRefs.current.clear();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [spaceId]);

  /** tab 切换后 fit 当前会话 */
  useEffect(() => {
    if (!activeKey) return;
    const entry = termRefs.current.get(activeKey);
    if (entry && entry.container) {
      // 等浏览器 layout 完
      requestAnimationFrame(() => {
        try {
          entry.fitAddon.fit();
        } catch {
          // ignore
        }
      });
    }
  }, [activeKey]);

  /** 窗口尺寸变化 → fit 当前 */
  useEffect(() => {
    function onWinResize() {
      if (!activeKey) return;
      const entry = termRefs.current.get(activeKey);
      if (entry && entry.container) {
        try {
          entry.fitAddon.fit();
        } catch {
          // ignore
        }
      }
    }
    window.addEventListener("resize", onWinResize);
    return () => window.removeEventListener("resize", onWinResize);
  }, [activeKey]);

  /** 把 xterm 挂载到容器 div（ref callback） */
  const attachContainer = useCallback(
    (sessionId: string) => (el: HTMLDivElement | null) => {
      const entry = termRefs.current.get(sessionId);
      if (!entry) return;
      if (el && entry.container !== el) {
        entry.container = el;
        // 只在未挂载过时 open
        if (!entry.term.element) {
          entry.term.open(el);
        } else {
          // xterm 已 open 过：把 element 移动到新容器
          el.appendChild(entry.term.element);
        }
        // 初次挂载后 fit
        requestAnimationFrame(() => {
          try {
            entry.fitAddon.fit();
          } catch {
            // ignore
          }
        });
      }
    },
    [],
  );

  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        height: "calc(100vh - 140px)",
        minHeight: 400,
      }}
    >
      {messageContextHolder}
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          marginBottom: 8,
        }}
      >
        <AntSpace>
          <span style={{ color: "#666", fontSize: 12 }}>
            {spaceName ? `空间：${spaceName}` : ""}
          </span>
          <Tag color="blue">{sessions.length} 个会话</Tag>
        </AntSpace>
        <Button size="small" icon={<PlusOutlined />} onClick={() => void createSession()}>
          新建终端
        </Button>
      </div>

      <Tabs
        type="editable-card"
        activeKey={activeKey}
        onChange={(k) => setActiveKey(k)}
        onEdit={(targetKey, action) => {
          if (action === "remove" && typeof targetKey === "string") {
            void closeSession(targetKey);
          } else if (action === "add") {
            void createSession();
          }
        }}
        hideAdd
        items={sessions.map((s) => ({
          key: s.sessionId,
          label: s.exited ? `${s.title}（已退出）` : s.title,
          closable: true,
        }))}
        style={{ flex: 1, minHeight: 0 }}
      />

      {/* 每个会话一个容器，display:none 切换 */}
      <div style={{ flex: 1, minHeight: 0, position: "relative", background: "#1e1e1e" }}>
        {sessions.map((s) => (
          <div
            key={s.sessionId}
            ref={attachContainer(s.sessionId)}
            style={{
              position: "absolute",
              inset: 0,
              display: s.sessionId === activeKey ? "block" : "none",
              padding: 4,
            }}
          />
        ))}
        {sessions.length === 0 && (
          <div
            style={{
              color: "#888",
              textAlign: "center",
              paddingTop: 80,
              fontSize: 13,
            }}
          >
            点击右上角"新建终端"开始
          </div>
        )}
      </div>
    </div>
  );
}
