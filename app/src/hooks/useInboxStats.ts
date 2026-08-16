import { useEffect, useRef, useState } from "react";
import { inboxStats } from "../api";

/**
 * 收件箱统计轮询 hook — m5-5.8 合并通知与角标
 *
 * 每 5 秒轮询 `inbox_stats`（与后端 5 秒聚合窗口对齐），
 * 返回最新 pending / snoozed / lastEventAt，以及上一轮的 pending（prevPending），
 * 便于 InboxNotification 计算增量、App 侧边栏角标实时刷新。
 *
 * 失败静默：保持上次值，避免打扰主流程。
 * 启动时不弹通知：prevPending 初始为 null，由消费方据此跳过首轮。
 */

/** 轮询间隔（毫秒）：5s，与后端聚合窗口对齐 */
export const INBOX_STATS_POLL_MS = 5_000;

export interface UseInboxStatsResult {
  pending: number;
  snoozed: number;
  /** Unix 秒；null 表示 inbox 为空 */
  lastEventAt: number | null;
  /**
   * 上一轮 pending 计数；首轮为 null（尚未有"上一次"）。
   * 消费方据此判断是否为启动首次拉取（不弹通知）。
   */
  prevPending: number | null;
}

interface StatsSnapshot {
  pending: number;
  snoozed: number;
  lastEventAt: number | null;
}

const INITIAL_STATS: StatsSnapshot = { pending: 0, snoozed: 0, lastEventAt: null };

export function useInboxStats(enabled: boolean): UseInboxStatsResult {
  const [stats, setStats] = useState<StatsSnapshot>(INITIAL_STATS);
  /** 上一轮 pending；首轮为 null（启动时不触发增量通知） */
  const prevPendingRef = useRef<number | null>(null);
  /** 是否已完成至少一次成功拉取；用于区分"启动首轮"与"后续轮次" */
  const hasFetchedRef = useRef(false);

  useEffect(() => {
    if (!enabled) return;
    let cancelled = false;

    const tick = async () => {
      try {
        const s = await inboxStats();
        if (cancelled) return;
        // 用 setStats 函数式更新读取上一次 state，同步写入 ref，
        // 再返回新 state —— 保证 prevPending 与 pending 在同一渲染帧内一致。
        setStats((prev) => {
          prevPendingRef.current = hasFetchedRef.current ? prev.pending : null;
          hasFetchedRef.current = true;
          return {
            pending: s.pending,
            snoozed: s.snoozed,
            lastEventAt: s.lastEventAt ?? null,
          };
        });
      } catch {
        // 静默：后端未就绪或网络异常时保持上次值
      }
    };

    void tick();
    const timer = window.setInterval(() => void tick(), INBOX_STATS_POLL_MS);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [enabled]);

  return {
    pending: stats.pending,
    snoozed: stats.snoozed,
    lastEventAt: stats.lastEventAt,
    prevPending: prevPendingRef.current,
  };
}
