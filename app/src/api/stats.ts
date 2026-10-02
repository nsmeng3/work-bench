import { invoke } from "@tauri-apps/api/core";

/** 按类型的引用计数条目 — `stats_overview` 出参 */
export interface TypeCount {
  type: string;
  count: number;
}

/** 近 7 天访问趋势条目 — `stats_overview` 出参 */
export interface DayCount {
  /** 本地日期 "MM-DD" */
  date: string;
  count: number;
}

/** `stats_overview` 出参（m7-7.6 统计页） */
export interface StatsOverview {
  spaceCount: number;
  collectionCount: number;
  referenceCount: number;
  watchDirCount: number;
  refsByType: TypeCount[];
  todoPending: number;
  todoDoing: number;
  todoDoneToday: number;
  todoDoneTotal: number;
  inboxPending: number;
  inboxSnoozed: number;
  accessLast7d: DayCount[];
}

/** `stats_overview ()` → StatsOverview。聚合各表计数，只读。 */
export async function statsOverview(): Promise<StatsOverview> {
  return invoke<StatsOverview>("stats_overview");
}
