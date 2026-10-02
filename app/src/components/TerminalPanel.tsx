import { useCallback, useEffect, useRef, useState } from "react";
import { Button, Space as AntSpace, Tabs, Tag, Tooltip, message } from "antd";
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
  terminalList,
  type TerminalEvent,
} from "../api";

/** 单个会话的全部运行时状态（xterm 实例 + 元信息）。 */
interface CachedSession {
  /** 后端 session_id */
  sessionId: string;
  /** tab 标题（"终端 1"） */
  title: string;
  /** 是否已退出（后端推了 Exited） */
  exited: boolean;
  term: Terminal;
  fitAddon: FitAddon;
  container: HTMLDivElement | null;
  /** 工作目录（创建后由 terminal_list 回填，用于 tab tooltip） */
  cwd?: string;
}

interface SpaceCache {
  sessions: CachedSession[];
  /** tab 标题序号 */
  seq: number;
  /** 当前激活的会话 key（存入缓存，跨挂载实例共享） */
  activeKey?: string;
  /** 创建中标记：防 React StrictMode 双挂载/快速连点导致重复创建 */
  creating?: boolean;
  /** 面板挂载期间用于触发 React 重渲染；卸载时置空 */
  onChange?: () => void;
}

/**
 * M7-4 · 跨页面会话缓存（模块级）。
 * 离开空间页只卸载 DOM，不销毁 PTY；回来时 xterm element 重新挂载、
 * 终端内容和后台进程原样保留。应用退出时由后端 kill_all 统一清理。
 */
const spaceCaches = new Map<string, SpaceCache>();

function getSpaceCache(spaceId: string): SpaceCache {
  let c = spaceCaches.get(spaceId);
  if (!c) {
    c = { sessions: [], seq: 0, activeKey: undefined };
    spaceCaches.set(spaceId, c);
  }
  return c;
}

/** 外部请求面板新建会话的信号（nonce 变化触发一次） */
export interface TerminalOpenRequest {
  /** 指定工作目录；缺省走后端兜底链（root_dir > home） */
  cwd?: string;
  nonce: number;
}

interface TerminalPanelProps {
  /** 会话分组键：空间页传 spaceId，全局终端页传固定值。
   *  同一 groupKey 的会话跨页面共享（模块级缓存）。 */
  groupKey: string;
  /** 关联空间（仅作后端会话元数据，可空） */
  spaceId?: string;
  spaceName?: string;
  /** 外部打开请求（如资源"在内嵌终端打开"） */
  openRequest?: TerminalOpenRequest | null;
}

/**
 * M7-4 · 内嵌终端面板。
 *
 * 关键设计：
 * - 会话状态存于模块级 spaceCaches，本组件只是它的渲染层；
 *   离开空间（unmount）不再杀会话，回来继续用。
 * - 每个会话一个容器 div，display:none 切换；
 *   ResizeObserver 监听容器尺寸（覆盖窗口缩放 / 外层 tab 显隐），自动 refit。
 * - 快捷键：Cmd/Ctrl+C 复制（有选区时）、Cmd/Ctrl+V 粘贴；
 *   无选区时 Ctrl+C 照常发 SIGINT。
 */
export function TerminalPanel({ groupKey, spaceId, spaceName, openRequest }: TerminalPanelProps) {
  const cache = getSpaceCache(groupKey);
  const [activeKey, setActiveKey] = useState<string | undefined>(
    () => cache.activeKey ?? cache.sessions[cache.sessions.length - 1]?.sessionId,
  );
  const [, setVersion] = useState(0);
  const [messageApi, messageContextHolder] = message.useMessage();

  const sync = useCallback(() => {
    setVersion((v) => v + 1);
    setActiveKey(cache.activeKey);
  }, [cache]);

  /** 切换 tab：写缓存（其他挂载实例同步）+ 本地状态 */
  const activate = useCallback(
    (key: string | undefined) => {
      cache.activeKey = key;
      setActiveKey(key);
    },
    [cache],
  );

  /** 创建一个新会话；cwd 指定时按该目录打开 */
  const createSession = useCallback(async (cwd?: string) => {
    if (cache.creating) return;
    cache.creating = true;
    try {
      cache.seq += 1;
      const title = `终端 ${cache.seq}`;

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

      // 先入缓存再异步创建 PTY；channel 闭包直接操作 cache，跨页面也安全
      const sess: CachedSession = {
        sessionId: "",
        title,
        exited: false,
        term,
        fitAddon,
        container: null,
        cwd,
      };

      const channel = new Channel<TerminalEvent>();
      channel.onmessage = (event) => {
        if (event.kind === "data") {
          term.write(event.data);
        } else if (event.kind === "exited") {
          term.write("\r\n\x1b[90m[进程已退出]\x1b[0m\r\n");
          sess.exited = true;
          cache.onChange?.();
        }
      };

      // 默认 80x24，挂载后 fitAddon.fit() 会触发 onResize 修正
      const out = await terminalCreate({ spaceId, cwd, cols: 80, rows: 24 }, channel);
      sess.sessionId = out.sessionId;

      // 回填实际 cwd（兜底链在后端，前端拿结果用于 tab tooltip）
      if (!sess.cwd) {
        void terminalList()
          .then((list) => {
            const item = list.find((i) => i.sessionId === out.sessionId);
            if (item) {
              sess.cwd = item.cwd;
              cache.onChange?.();
            }
          })
          .catch(() => void 0);
      }

      term.onData((data) => {
        void terminalWrite(out.sessionId, data).catch(() => void 0);
      });
      term.onResize(({ cols, rows }) => {
        void terminalResize(out.sessionId, cols, rows).catch(() => void 0);
      });

      // 复制粘贴：有选区时 Cmd/Ctrl+C 复制；Cmd/Ctrl+V 粘贴。
      // 无选区的 Ctrl+C 返回 true，交给 xterm 发 SIGINT。
      term.attachCustomKeyEventHandler((e) => {
        if (e.type !== "keydown") return true;
        const mod = e.metaKey || e.ctrlKey;
        const key = e.key.toLowerCase();
        if (mod && key === "c" && term.hasSelection()) {
          void navigator.clipboard.writeText(term.getSelection()).catch(() => void 0);
          return false;
        }
        if (mod && key === "v") {
          void navigator.clipboard
            .readText()
            .then((text) => {
              if (text) void terminalWrite(out.sessionId, text).catch(() => void 0);
            })
            .catch(() => void 0);
          return false;
        }
        return true;
      });

      cache.sessions.push(sess);
      cache.activeKey = out.sessionId;
      cache.onChange?.();
    } catch (err) {
      messageApi.error(`创建终端失败: ${String(err)}`);
    } finally {
      cache.creating = false;
    }
  }, [cache, spaceId, messageApi]);

  /** 关闭会话：kill PTY + dispose xterm + 从缓存移除 */
  const closeSession = useCallback(
    async (sessionId: string) => {
      try {
        await terminalClose(sessionId);
      } catch {
        // 已关闭也继续走本地清理
      }
      const idx = cache.sessions.findIndex((s) => s.sessionId === sessionId);
      if (idx >= 0) {
        const [sess] = cache.sessions.splice(idx, 1);
        sess.term.dispose();
      }
      refCallbacks.current.delete(sessionId);
      if (cache.activeKey === sessionId) {
        cache.activeKey = cache.sessions[cache.sessions.length - 1]?.sessionId;
      }
      cache.onChange?.();
      // 面板未挂载时 onChange 为空，本地状态也要落
      setActiveKey(cache.activeKey);
    },
    [cache],
  );

  /** 挂载：接管 cache 的重渲染回调；首次进入且无会话时自动开一个 */
  useEffect(() => {
    cache.onChange = sync;
    if (cache.sessions.length === 0 && !cache.creating) {
      void createSession();
    } else {
      // 从其他页面回来：主动同步一次（离开期间可能有 Exited / 创建完成事件）
      sync();
    }
    return () => {
      // 只卸载 DOM，不销毁会话
      cache.onChange = undefined;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [cache, sync]);

  /** 外部打开请求：nonce 变化 → 新建会话（可带 cwd） */
  useEffect(() => {
    if (openRequest) void createSession(openRequest.cwd);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [openRequest?.nonce]);

  /** 当前会话容器尺寸变化（窗口缩放 / 外层 tab 显隐）→ refit */
  useEffect(() => {
    if (!activeKey) return;
    const sess = cache.sessions.find((s) => s.sessionId === activeKey);
    const el = sess?.container;
    if (!sess || !el) return;

    const fit = () => {
      // display:none 时尺寸为 0，fit 会产生非法 cols/rows，跳过
      if (el.clientWidth === 0 || el.clientHeight === 0) return;
      try {
        sess.fitAddon.fit();
      } catch {
        // ignore
      }
    };
    const raf = requestAnimationFrame(fit);
    const ro = new ResizeObserver(fit);
    ro.observe(el);
    return () => {
      cancelAnimationFrame(raf);
      ro.disconnect();
    };
  }, [activeKey, cache]);

  /**
   * 按会话缓存的 ref 回调工厂。
   * 关键：ref 回调必须引用稳定，否则每次渲染 React 都会先 ref(null) 再
   * ref(el)，attachContainer 会把 xterm DOM 摘下重挂 → 内部 textarea
   * 失焦 → 键重复中断（"按住回车刷几行就停住"的根因）。
   */
  const refCallbacks = useRef(new Map<string, (el: HTMLDivElement | null) => void>());

  /** 把 xterm 挂载到容器 div（ref callback）；卸载时只释放容器引用 */
  const attachContainer = useCallback(
    (sessionId: string) => (el: HTMLDivElement | null) => {
      const sess = cache.sessions.find((s) => s.sessionId === sessionId);
      if (!sess) return;
      if (!el) {
        sess.container = null;
        return;
      }
      if (sess.container === el) return;
      sess.container = el;
      if (!sess.term.element) {
        sess.term.open(el);
      } else {
        // xterm 已 open 过：把 element 移动到新容器
        el.appendChild(sess.term.element);
      }
      requestAnimationFrame(() => {
        if (el.clientWidth === 0 || el.clientHeight === 0) return;
        try {
          sess.fitAddon.fit();
        } catch {
          // ignore
        }
      });
    },
    [cache],
  );

  const sessions = cache.sessions;

  /** 取会话的稳定 ref 回调（不存在则创建并缓存） */
  const getRefCallback = (sessionId: string) => {
    let cb = refCallbacks.current.get(sessionId);
    if (!cb) {
      cb = attachContainer(sessionId);
      refCallbacks.current.set(sessionId, cb);
    }
    return cb;
  };

  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        height: "100%",
        minHeight: 0,
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
        onChange={(k) => activate(k)}
        onEdit={(targetKey, action) => {
          if (action === "remove" && typeof targetKey === "string") {
            void closeSession(targetKey);
          } else if (action === "add") {
            void createSession();
          }
        }}
        hideAdd
        items={sessions.map((s) => ({
          key: s.sessionId || s.title,
          label: (
            <Tooltip title={s.cwd} placement="bottom">
              {s.exited ? `${s.title}（已退出）` : s.title}
            </Tooltip>
          ),
          closable: true,
        }))}
        style={{ flex: "none", marginBottom: 4 }}
      />

      {/* 每个会话一个容器，display:none 切换 */}
      <div style={{ flex: 1, minHeight: 0, position: "relative", background: "#1e1e1e" }}>
        {sessions.map((s) => (
          <div
            key={s.sessionId || s.title}
            ref={getRefCallback(s.sessionId || s.title)}
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
