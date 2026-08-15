import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";

/**
 * managed_progress 事件载荷 — 契约冻结（详细设计说明书 §4.1 / 任务包 m3-3.7）。
 *
 * 后端节流约定（3.3）：每复制满 1 MiB 或每 200ms 发一次；完成时发最后一次
 * `bytes == total`（或省略由 confirmed 命令返回代表完成 —— 前端两种都兼容）。
 */
export interface ManagedProgressPayload {
  refId: string;
  bytes: number;
  total: number;
}

/** 单个托管任务的进度快照 */
export interface ManagedProgressEntry {
  bytes: number;
  total: number;
  /** 最近一次事件到达时间（ms），便于 UI 判断"是否仍在推进" */
  updatedAt: number;
}

/**
 * useManagedProgress — 订阅 Tauri `managed_progress` 事件，按 refId 维护进度 map。
 *
 * 设计要点：
 * - 多任务区分：同一时刻可能有多个"导入并托管"任务并发，使用 refId 作为 key。
 * - 组件卸载时自动 unlisten，避免泄漏。
 * - 进度条不真正中断后端：后端无取消机制（契约冻结），UI 取消仅隐藏进度，
 *   后台复制继续完成。调用方需在注释中固化此已知限制。
 *
 * 复用：M4 处置三档（软删/销毁）涉及大文件操作时可直接复用本 hook。
 */
export function useManagedProgress() {
  const [progressMap, setProgressMap] = useState<Record<string, ManagedProgressEntry>>({});
  // 用 ref 保存 setProgressMap 的稳定性，避免 effect 重复订阅
  const mountedRef = useRef(true);

  useEffect(() => {
    mountedRef.current = true;
    let unlisten: (() => void) | null = null;
    let cancelled = false;

    listen<ManagedProgressPayload>("managed_progress", (event) => {
      if (!mountedRef.current) return;
      const { refId, bytes, total } = event.payload;
      if (!refId) return;
      setProgressMap((prev) => ({
        ...prev,
        [refId]: { bytes, total, updatedAt: Date.now() },
      }));
    })
      .then((fn) => {
        if (cancelled) {
          // 在 listen resolve 之前组件已卸载 —— 立即解绑
          fn();
        } else {
          unlisten = fn;
        }
      })
      .catch((err) => {
        // 非 Tauri 环境（如纯 web 调试）会失败，降级为无进度展示
        console.warn("[useManagedProgress] listen managed_progress 失败：", err);
      });

    return () => {
      cancelled = true;
      mountedRef.current = false;
      if (unlisten) unlisten();
    };
  }, []);

  /** 读取指定 refId 的进度；不存在返回 null */
  const get = (refId: string | null | undefined): ManagedProgressEntry | null => {
    if (!refId) return null;
    return progressMap[refId] ?? null;
  };

  /** 清除指定 refId 的进度（任务结束/取消后调用，避免 map 膨胀） */
  const clear = (refId: string) => {
    setProgressMap((prev) => {
      if (!(refId in prev)) return prev;
      const next = { ...prev };
      delete next[refId];
      return next;
    });
  };

  return { progressMap, get, clear };
}
