import { invoke } from "@tauri-apps/api/core";
import type {
  InboxItem,
  InboxItemDetail,
  InboxListInput,
  InboxSnoozeInput,
  InboxIgnoreInput,
  InboxAssignInput,
  InboxAssignResult,
  InboxDismissStaleInput,
  InboxStats,
  WatchDirConfig,
} from "./types";
import { mockInboxApi } from "./mock";

const MOCK = import.meta.env.VITE_MOCK_API === "true";

/**
 * 收件箱 API — 契约见详细设计说明书 §2.7。
 * 后端 5.4 已交付 inbox_list / inbox_get / inbox_snooze / inbox_ignore / inbox_stats；
 * 后端未就绪时可通过 VITE_MOCK_API=true 切换到 mock 数据。
 *
 * 关键约束（§6.9）：敏感文件 sensitiveWarning 非空时前端不显示预览。
 */

/** `inbox_list { status?, limit?, offset? }` → `InboxItem[]` */
export async function inboxList(input: InboxListInput = {}): Promise<InboxItem[]> {
  if (MOCK) return mockInboxApi.inbox_list(input);
  return invoke<InboxItem[]>("inbox_list", { ...input });
}

/** `inbox_get { id }` → `InboxItemDetail`（含可选预览与敏感提示） */
export async function inboxGet(id: string): Promise<InboxItemDetail> {
  if (MOCK) return mockInboxApi.inbox_get(id);
  return invoke<InboxItemDetail>("inbox_get", { id });
}

/** `inbox_snooze { id, note?, remindAt? }` → `InboxItem`（status → snoozed） */
export async function inboxSnooze(input: InboxSnoozeInput): Promise<InboxItem> {
  if (MOCK) return mockInboxApi.inbox_snooze(input);
  return invoke<InboxItem>("inbox_snooze", { ...input });
}

/** `inbox_ignore { id, rule? }` → `InboxItem`（status → ignored） */
export async function inboxIgnore(input: InboxIgnoreInput): Promise<InboxItem> {
  if (MOCK) return mockInboxApi.inbox_ignore(input);
  return invoke<InboxItem>("inbox_ignore", { ...input });
}

/** `inbox_stats ()` → `{ pending, snoozed, lastEventAt }`（用于侧边栏角标） */
export async function inboxStats(): Promise<InboxStats> {
  if (MOCK) return mockInboxApi.inbox_stats();
  return invoke<InboxStats>("inbox_stats");
}

/**
 * `inbox_assign { id, mode, spaceId, collectionId, type, managedAction?, targetName?, confirmed }`
 * → `InboxAssignResult`（§2.7）。
 * - external：confirmed=true，直接转正式引用
 * - managed confirmed=false：返回 managedPlan 用于二次确认
 * - managed confirmed=true：执行导入并托管
 */
export async function inboxAssign(input: InboxAssignInput): Promise<InboxAssignResult> {
  if (MOCK) return mockInboxApi.inbox_assign(input);
  return invoke<InboxAssignResult>("inbox_assign", { ...input });
}

/**
 * `inbox_dismiss_stale { id }` → `InboxItem`（status → processed）。
 * 用于源文件已不在（INBOX_STALE）时用户主动标记为已处理。
 */
export async function inboxDismissStale(input: InboxDismissStaleInput): Promise<InboxItem> {
  if (MOCK) return mockInboxApi.inbox_dismiss_stale(input);
  return invoke<InboxItem>("inbox_dismiss_stale", { ...input });
}

/**
 * 监控目录命令 — M6-6.1 声明契约
 */
/** `watch_dir_get ()` → `WatchDirConfig[]`（按 path 升序） */
export async function inboxGetWatchDirs(): Promise<WatchDirConfig[]> {
  if (MOCK) return mockInboxApi.inbox_get_watch_dirs();
  return invoke<WatchDirConfig[]>("watch_dir_get");
}

/**
 * `watch_dir_set { path, name, description? }` → `WatchDirConfig`。
 * 路径校验：必须为绝对路径。
 */
export async function inboxSetWatchDir(input: WatchDirConfig): Promise<WatchDirConfig> {
  if (MOCK) return mockInboxApi.inbox_set_watch_dir(input);
  return invoke<WatchDirConfig>("watch_dir_set", { input });
}

/**
 * `watch_dir_unset { path }` → `WatchDirConfig`。
 */
export async function inboxUnsetWatchDir(input: { path: string }): Promise<WatchDirConfig> {
  if (MOCK) return mockInboxApi.inbox_unset_watch_dir(input);
  return invoke<WatchDirConfig>("watch_dir_unset", { path: input.path });
}
