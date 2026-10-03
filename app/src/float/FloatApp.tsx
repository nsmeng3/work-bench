import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { CloseOutlined, InboxOutlined } from "@ant-design/icons";

/**
 * 悬浮通知窗（应用统一通知通道，约定见 CLAUDE.md）
 * 独立 Tauri 窗口，URL 带 ?float=1 时渲染本组件。
 *
 * - 窗口本身无边框、透明、置顶；本组件渲染一张圆角卡片。
 * - 收到 Rust 侧 float_notify 发来的 "float-data" 事件后显示，
 *   AUTO_HIDE_MS 后自动隐藏（新事件会重置倒计时并替换内容）。
 * - 点卡片 → float_click：唤起主窗口，并按 payload.action 向主窗口
 *   emit 对应事件（如收件箱的 "go-inbox"）；点 × → float_hide 仅关闭本次提醒。
 */

/** 自动隐藏时长（毫秒） */
const AUTO_HIDE_MS = 6_000;

/** float_notify 的负载（对齐 Rust FloatPayload） */
interface FloatPayload {
  title: string;
  body: string;
  /** 点击卡片后向主窗口 emit 的事件名；缺省表示点击仅唤起主窗口 */
  action?: string | null;
}

export default function FloatApp() {
  /** 当前通知内容；null 表示不渲染（保持窗口全透明） */
  const [payload, setPayload] = useState<FloatPayload | null>(null);
  const timerRef = useRef<number | null>(null);

  const clearTimer = () => {
    if (timerRef.current !== null) {
      window.clearTimeout(timerRef.current);
      timerRef.current = null;
    }
  };

  const hide = useCallback(() => {
    clearTimer();
    setPayload(null);
    void invoke("float_hide").catch(() => {});
  }, []);

  useEffect(() => {
    // 覆盖 theme.css 的底色，让窗口真正透明（只显示卡片）
    document.documentElement.style.background = "transparent";
    document.body.style.background = "transparent";

    const unlisten = listen<FloatPayload>("float-data", (e) => {
      setPayload(e.payload);
      clearTimer();
      timerRef.current = window.setTimeout(hide, AUTO_HIDE_MS);
    });
    return () => {
      void unlisten.then((f) => f());
      clearTimer();
    };
  }, [hide]);

  if (payload === null) return null;

  return (
    <div
      onClick={() =>
        void invoke("float_click", { action: payload.action ?? null }).catch(() => {})
      }
      style={{
        display: "flex",
        alignItems: "center",
        gap: 12,
        height: "100vh",
        boxSizing: "border-box",
        padding: "0 16px",
        borderRadius: 12,
        background: "rgba(32, 33, 36, 0.92)",
        boxShadow: "0 6px 24px rgba(0, 0, 0, 0.35)",
        color: "#fff",
        cursor: "pointer",
        userSelect: "none",
        fontFamily:
          '-apple-system, BlinkMacSystemFont, "PingFang SC", "Helvetica Neue", sans-serif',
      }}
    >
      <InboxOutlined style={{ fontSize: 28, color: "#4a90d9" }} />
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ fontSize: 14, fontWeight: 600 }}>{payload.title}</div>
        <div
          style={{
            fontSize: 12,
            opacity: 0.85,
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
          }}
        >
          {payload.body}
        </div>
      </div>
      <CloseOutlined
        style={{ fontSize: 12, opacity: 0.6 }}
        onClick={(e) => {
          e.stopPropagation();
          hide();
        }}
      />
    </div>
  );
}
