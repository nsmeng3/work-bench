import { useEffect, useRef } from "react";
import { App, Button, notification } from "antd";

/**
 * 收件箱合并通知 — m5-5.8
 *
 * 监听 useInboxStats 返回的 pending / prevPending：
 * - 当 pending > prevPending 时弹出 antd notification.info，
 *   文案「收件箱新增 N 条待处理」（N = pending - prevPending）。
 * - 「前往查看」按钮点击后跳转收件箱（由父组件通过 onGoInbox 注入，
 *   通常是切换 activeNav 到 "inbox"）。
 * - 5 秒内多次增量只弹一次：用 lastShownAtRef 记录上次弹窗时间，
 *   距上次不足 5 秒则跳过；累计增量并入下次文案。
 * - 应用启动时不弹：prevPending 为 null 表示首轮拉取，直接跳过。
 *
 * 渲染 null，仅承担副作用。
 */

/** 合并窗口（毫秒）：距上次弹窗不足此时长则跳过本次，把增量并入下次 */
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
  /** 上次弹窗时间戳（performance.now 语义，毫秒）；0 表示尚未弹过 */
  const lastShownAtRef = useRef(0);
  /** 在合并窗口内被合并掉的增量累计；下次弹窗时一并展示 */
  const pendingDeltaRef = useRef(0);

  useEffect(() => {
    // 首轮：prevPending 为 null → 不弹
    if (prevPending === null) return;
    const delta = pending - prevPending;
    if (delta <= 0) return;

    const now = Date.now();
    const sinceLast = now - lastShownAtRef.current;

    if (sinceLast < MERGE_WINDOW_MS) {
      // 合并窗口内：累计增量，不弹
      pendingDeltaRef.current += delta;
      return;
    }

    // 窗口外：本次 delta + 之前累计的合并增量一起展示
    const total = delta + pendingDeltaRef.current;
    pendingDeltaRef.current = 0;
    lastShownAtRef.current = now;

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
  }, [pending, prevPending, api, onGoInbox]);

  return null;
}
