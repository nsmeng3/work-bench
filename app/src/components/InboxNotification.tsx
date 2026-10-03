import { useEffect, useRef } from "react";
import { App, Button, notification } from "antd";
import { invoke } from "@tauri-apps/api/core";

/**
 * 收件箱合并通知 — m5-5.8
 *
 * 监听 useInboxStats 返回的 pending / prevPending：
 * - 当 pending > prevPending 时弹出 antd notification.info，
 *   文案「收件箱新增 N 条待处理」（N = pending - prevPending）。
 * - 「前往查看」按钮点击后跳转收件箱（由父组件通过 onGoInbox 注入，
 *   通常是切换 activeNav 到 "inbox"）。
 * - 5 秒内多次增量只弹一次：距上次弹窗不足 5 秒的增量先累计到
 *   pendingDeltaRef，并安排一个兜底 flush 定时器——窗口到期后即使
 *   没有新的增量，累计增量也会弹出（旧逻辑只靠「下次窗口外增量」
 *   带出，若之后没有新增量，这次通知会被永久吞掉）。
 * - 应用启动时不弹：prevPending 为 null 表示首轮拉取，直接跳过。
 *
 * 渲染 null，仅承担副作用。
 */

/** 合并窗口（毫秒）：距上次弹窗不足此时长则累计，窗口到期由兜底定时器弹出 */
const MERGE_WINDOW_MS = 5_000;

const NOTIFICATION_KEY = "inbox-merge-notify";

interface InboxNotificationProps {
  pending: number;
  prevPending: number | null;
  /** 点击「前往查看」回调；通常切换 activeNav 到 "inbox" 并关闭通知 */
  onGoInbox: () => void;
}

export function InboxNotification({ pending, prevPending, onGoInbox }: InboxNotificationProps) {
  // antd v5 推荐通过 App 组件上下文使用 notification，避免静态函数警告
  const { notification: api } = App.useApp();
  /** 上次弹窗时间戳（Date.now 语义，毫秒）；0 表示尚未弹过 */
  const lastShownAtRef = useRef(0);
  /** 在合并窗口内被累计的增量；窗口外弹窗或兜底 flush 时一并展示 */
  const pendingDeltaRef = useRef(0);
  /** 合并窗口兜底 flush 定时器（null 表示未安排） */
  const flushTimerRef = useRef<number | null>(null);

  // 卸载时清理兜底定时器，避免组件销毁后弹出通知
  useEffect(() => {
    return () => {
      if (flushTimerRef.current !== null) {
        window.clearTimeout(flushTimerRef.current);
        flushTimerRef.current = null;
      }
    };
  }, []);

  useEffect(() => {
    /** 弹出合并通知：清掉待执行的兜底 flush，重置累计增量 */
    const show = (total: number) => {
      if (flushTimerRef.current !== null) {
        window.clearTimeout(flushTimerRef.current);
        flushTimerRef.current = null;
      }
      pendingDeltaRef.current = 0;
      lastShownAtRef.current = Date.now();

      // 悬浮通知窗（统一通知通道，见 CLAUDE.md 约定）：复用合并窗口的 total，
      // 失败静默（MOCK/浏览器环境无 Tauri）
      void invoke("float_notify", {
        title: "目录监控",
        body: `收件箱新增 ${total} 条待处理，点击查看`,
        action: "go-inbox",
      }).catch((err) => {
        console.warn("[InboxNotification] 悬浮窗通知不可用（非 Tauri 环境）", err);
      });

      api.info({
        key: NOTIFICATION_KEY,
        message: "收件箱有新待处理",
        description: `收件箱新增 ${total} 条待处理`,
        placement: "topRight",
        duration: 4.5,
        btn: (
          <Button
            type="primary"
            size="small"
            onClick={() => {
              notification.destroy(NOTIFICATION_KEY);
              onGoInbox();
            }}
          >
            前往查看
          </Button>
        ),
      });
    };

    // 首轮：prevPending 为 null → 不弹
    if (prevPending === null) return;
    const delta = pending - prevPending;
    if (delta <= 0) return;

    const now = Date.now();
    const sinceLast = now - lastShownAtRef.current;

    if (sinceLast < MERGE_WINDOW_MS) {
      // 合并窗口内：累计增量，并安排兜底 flush（尚无定时器才安排，
      // 避免重复）；窗口到期后即使没有新增量也会弹出
      pendingDeltaRef.current += delta;
      if (flushTimerRef.current === null) {
        const wait = MERGE_WINDOW_MS - sinceLast;
        flushTimerRef.current = window.setTimeout(() => {
          flushTimerRef.current = null;
          const total = pendingDeltaRef.current;
          if (total > 0) show(total);
        }, wait);
      }
      return;
    }

    // 窗口外：本次 delta + 之前累计的合并增量一起展示
    show(delta + pendingDeltaRef.current);
  }, [pending, prevPending, api, onGoInbox]);

  return null;
}
