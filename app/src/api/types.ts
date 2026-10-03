/**
 * 统一错误结构 — 详细设计说明书 §2.2
 */
export interface ApiError {
  code: string;
  message: string;
  details?: Record<string, unknown>;
  retryable: boolean;
}

/**
 * 空间实体 — 详细设计说明书 §2.3 / §3.2 space 表
 * createdAt / updatedAt 为 Unix 秒（number）。
 */
export interface Space {
  id: string;
  name: string;
  description?: string;
  color?: string;
  icon?: string;
  status: "active" | "archived";
  createdAt: number;
  updatedAt: number;
}

/** space_create 入参 */
export interface SpaceCreateInput {
  name: string;
  description?: string;
  color?: string;
  icon?: string;
}

/** space_update 入参 */
export interface SpaceUpdateInput {
  id: string;
  name?: string;
  description?: string;
  color?: string;
  icon?: string;
}

/** space_archive / space_restore 入参 */
export interface SpaceIdInput {
  id: string;
}

/** space_list 入参 */
export interface SpaceListInput {
  status?: "active" | "archived" | "all";
}

/**
 * 资源集实体 — 详细设计说明书 §2.4 / §3.2 collection 表。
 * createdAt / updatedAt 为 Unix 秒（number）。
 */
export interface Collection {
  id: string;
  spaceId: string;
  name: string;
  summary?: string;
  tags?: string[];
  status: "active" | "archived";
  createdAt: number;
  updatedAt: number;
}

/** `collection_recent_access` 出参条目（Dashboard「最近资源集」卡片） */
export interface RecentCollection {
  id: string;
  spaceId: string;
  spaceName: string;
  name: string;
  summary?: string;
  /** 资源集内 active 资源数 */
  refCount: number;
  /** Unix 秒：集内资源最近一次被访问的时间 */
  lastAt: number;
}

/** collection_create 入参 */
export interface CollectionCreateInput {
  spaceId: string;
  name: string;
  summary?: string;
  tags?: string[];
}

/** collection_update 入参 */
export interface CollectionUpdateInput {
  id: string;
  name?: string;
  summary?: string;
  tags?: string[];
}

/** collection_archive / collection_restore / collection_get 入参 */
export interface CollectionIdInput {
  id: string;
}

/** collection_list 入参 */
export interface CollectionListInput {
  spaceId: string;
  status?: "active" | "archived" | "all";
}

/* ---------------- 资源引用（Reference） ---------------- */

/** 引用类型 — 详细设计说明书 §2.5 / §3.2 resource_reference 表 */
export type ReferenceType = "code" | "document" | "data" | "artifact" | "tool" | "media";

/** 引用健康度 — §2.4 collection_get.referencesByType.*.health */
export type ReferenceHealth = "ok" | "missing" | "unknown";

/** 引用生命周期 */
export type ReferenceLifecycle = "active" | "staged" | "delivered" | "archived";

/** 引用保密级别 */
export type ReferenceConfidentiality =
  | "public"
  | "internal"
  | "customer_restricted"
  | "sensitive";

/** 引用托管方式 */
export type ReferenceHosting = "external" | "managed";

/** 引用处置状态 */
export type ReferenceDisposition = "none" | "archived" | "deleted";

/** 引用定位（locator）— §3.2 resource_reference.locator_json */
export type ReferenceLocator =
  | { kind: "path"; path: string; originalSource?: string; managedAction?: ManagedAction }
  | { kind: "repo"; local: string; remote?: string; defaultBranch?: string }
  | { kind: "cloud"; provider: string; objectId: string };

/**
 * 资源引用实体 — §2.5 / §3.2 resource_reference 表。
 * createdAt / updatedAt 为 Unix 秒（number）。
 */
export interface Reference {
  id: string;
  collectionId: string;
  sourceId: string;
  name: string;
  type: ReferenceType;
  hosting: ReferenceHosting;
  locator: ReferenceLocator;
  description?: string;
  tags?: string[];
  lifecycle: ReferenceLifecycle;
  confidentiality: ReferenceConfidentiality;
  indexed: boolean;
  disposition: ReferenceDisposition;
  createdAt: number;
  updatedAt: number;
}

/** collection_get.referencesByType 中每个引用条目携帯健康度 */
export interface ReferenceWithHealth {
  ref: Reference;
  health: ReferenceHealth;
}

/**
 * ref_update 入参 — §2.5：仅允许修改管理属性；
 * type / locator / hosting 不可改，故不在此出现。
 * 所有字段可选，仅传入需要修改的字段；tags 为全量替换语义。
 */
export interface RefUpdateInput {
  id: string;
  name?: string;
  description?: string;
  tags?: string[];
  lifecycle?: ReferenceLifecycle;
  confidentiality?: ReferenceConfidentiality;
  indexed?: boolean;
}

/** collection_get 出参 — §2.4 CollectionDetail */
export interface CollectionDetail {
  id: string;
  spaceId: string;
  name: string;
  summary?: string;
  tags?: string[];
  status: "active" | "archived";
  createdAt: number;
  updatedAt: number;
  /** 六类型分组；固定键序 code/document/data/artifact/tool/media */
  referencesByType: Record<ReferenceType, ReferenceWithHealth[]>;
}

/** ref_create_external 入参 — 详细设计说明书 §2.5 */
export interface RefCreateExternalInput {
  collectionId: string;
  name: string;
  type: ReferenceType;
  locator: ReferenceLocator;
  description?: string;
  tags?: string[];
  lifecycle?: ReferenceLifecycle;
  confidentiality?: ReferenceConfidentiality;
  indexed?: boolean;
}

/* ---------------- 导入并托管（§2.5 ref_create_managed） ---------------- */

/** 托管动作：copy 保留源 / move 移动（源将被删除） */
export type ManagedAction = "copy" | "move";

/**
 * ref_create_managed 入参 — 详细设计说明书 §2.5。
 * 基础字段与 ref_create_external 相同，额外加 managedAction / targetName / confirmed。
 * confirmed=false 拿 ManagedPlan；confirmed=true 执行落地返回 Reference。
 */
export interface RefCreateManagedInput {
  collectionId: string;
  name: string;
  type: ReferenceType;
  locator: ReferenceLocator;
  description?: string;
  tags?: string[];
  lifecycle?: ReferenceLifecycle;
  confidentiality?: ReferenceConfidentiality;
  indexed?: boolean;
  managedAction: ManagedAction;
  /** 自定义目标名（缺省用源名）；仅在 confirmed=true 阶段生效 */
  targetName?: string;
  confirmed: boolean;
}

/**
 * ManagedPlan — ref_create_managed(confirmed=false) 出参。
 * 不执行任何写操作；前端据此渲染确认框。
 */
export interface ManagedPlan {
  kind: "managed_plan";
  /** 源绝对路径 */
  source: string;
  /** 后端建议的目标绝对路径（rootDir/类型子目录/源名） */
  proposedTarget: string;
  /** 当前请求的托管动作（回显） */
  action: ManagedAction;
  /** 总字节数 */
  sizeBytes: number;
  /** 文件数；单文件通常为 1，目录为递归文件总数 */
  fileCount: number;
  /** 冲突描述列表；非空时前端禁用「确认」直至用户改 targetName */
  conflicts: string[];
}

/* ---------------- 查询筛选（§2.9） ---------------- */

/** query_refs 入参 — 全部可选；tags 为 AND 语义 */
export interface QueryRefsInput {
  spaceId?: string;
  collectionId?: string;
  type?: ReferenceType;
  tags?: string[];
  lifecycle?: ReferenceLifecycle;
  confidentiality?: ReferenceConfidentiality;
  disposition?: ReferenceDisposition;
  sourceId?: string;
  keyword?: string;
  limit?: number;
  offset?: number;
}

/** query_refs 出参 */
export interface QueryRefsOutput {
  total: number;
  items: Reference[];
}

/** query_facets 入参 */
export interface QueryFacetsInput {
  spaceId?: string;
}

/** 单个 facet 可选值与计数 */
export interface FacetValue {
  value: string;
  count: number;
}

/** query_facets 出参 — 四个维度的可用筛选值与计数 */
export interface QueryFacetsOutput {
  types: FacetValue[];
  lifecycles: FacetValue[];
  confidentialities: FacetValue[];
  tags: FacetValue[];
}

/* ---------------- 设置中心（§2.8） ---------------- */

/** settings_get_root_dir 出参 — §2.8 */
export interface RootDirStatus {
  rootDir?: string;
  initialized: boolean;
}

/** settings_init_root_dir 入参 — §2.8 */
export interface InitRootDirInput {
  rootDir: string;
}

/** settings_init_root_dir 出参 — §2.8 */
export interface InitRootDirResult {
  rootDir: string;
  created: string[];
}

/** 存储源信息 — §2.8 settings_list_sources 出参 */
export interface StorageSourceInfo {
  id: string;
  name: string;
  kind: string;
  status: string;
  capabilities: {
    archive: boolean;
    softDelete: boolean;
    destroy: boolean;
    restoreFromBin: boolean;
  };
}

/** settings_update_source 入参 — §2.8 */
export interface StorageSourceUpdateInput {
  id: string;
  name?: string;
}

/** 默认程序配置 — §2.8 settings_get_default_app / settings_set_default_app */
export interface DefaultAppConfig {
  type: string;
  strategy: "system_default" | "app";
  appPath?: string;
}

/** settings_set_default_app 入参 — §2.8 */
export interface DefaultAppSetInput {
  type: string;
  strategy: "system_default" | "app";
  appPath?: string;
}

/* ---------------- 修改根目录（§2.8 settings_change_root_dir 两阶段） ---------------- */

/**
 * 修改根目录策略 — §2.8 / §4.4。
 * - future_only：仅改 settings.root_dir，不动已有文件与引用。
 * - migrate：逐项复制 + 更新 locator_json。
 */
export type ChangeRootStrategy = "future_only" | "migrate";

/** settings_change_root_dir 入参 */
export interface SettingsChangeRootDirInput {
  newRootDir: string;
  strategy: ChangeRootStrategy;
  confirmed: boolean;
}

/** MigrationPlan.items 单项状态 */
export type MigrationItemStatus = "ok" | "source_missing" | "target_conflict";

/** MigrationPlan.items 单项（§2.8） */
export interface MigrationPlanItem {
  refId: string;
  currentPath: string;
  proposedPath: string;
  status: MigrationItemStatus;
}

/** confirmed=false 时 settings_change_root_dir 的出参（§2.8 MigrationPlan） */
export interface MigrationPlan {
  newRootDir: string;
  strategy: ChangeRootStrategy;
  items: MigrationPlanItem[];
  /** 可迁移项（status == "ok"）源文件总字节数 */
  totalBytes: number;
  /** 目标冲突的 proposedPath 列表 */
  conflicts: string[];
}

/** migrate 单项失败记录 */
export interface MigrationFailure {
  refId: string;
  currentPath: string;
  proposedPath: string;
  /** 人类可读失败原因 */
  error: string;
}

/** confirmed=true && strategy="migrate" 出参（§2.8 MigrationResult） */
export interface MigrationResult {
  migrated: string[];
  failed: MigrationFailure[];
}

/** confirmed=true && strategy="future_only" 出参 */
export interface FutureOnlyResult {
  newRootDir: string;
}

/**
 * settings_change_root_dir 两阶段出参（untagged 联合）。
 * 通过字段鉴别：
 * - 含 `items` → MigrationPlan
 * - 含 `migrated` / `failed` → MigrationResult
 * - 仅 `newRootDir` → FutureOnlyResult
 */
export type ChangeRootResult = MigrationPlan | MigrationResult | FutureOnlyResult;

/** 类型守卫：MigrationPlan */
export function isMigrationPlan(r: ChangeRootResult): r is MigrationPlan {
  return Array.isArray((r as MigrationPlan).items);
}

/** 类型守卫：MigrationResult */
export function isMigrationResult(r: ChangeRootResult): r is MigrationResult {
  return Array.isArray((r as MigrationResult).migrated);
}

/** 类型守卫：FutureOnlyResult */
export function isFutureOnlyResult(r: ChangeRootResult): r is FutureOnlyResult {
  return (
    !isMigrationPlan(r) &&
    !isMigrationResult(r) &&
    typeof (r as FutureOnlyResult).newRootDir === "string"
  );
}

/* ---------------- 处置审计（§2.6 disp_audit_list） ---------------- */

/** 审计 action 合法值 — 与 disposition_audit.action CHECK 约束一致（m4-4.9 加入 undo_import） */
export type DispositionAuditAction = "archive" | "unarchive" | "soft_delete" | "destroy" | "undo_import";

/**
 * 处置审计条目 — 详细设计说明书 §2.6 disp_audit_list 出参 / §3.2 disposition_audit 表。
 * `at` 为 Unix 秒（number）。
 * `locatorSnapshot` 为反序列化后的 JSON 对象；写入时若 locator_json 解析失败可能为 null。
 */
export interface DispositionAudit {
  id: string;
  refId: string;
  refName: string;
  action: DispositionAuditAction;
  locatorSnapshot: ReferenceLocator | null;
  actor: string;
  note?: string | null;
  at: number;
}

/** disp_audit_list 入参 — 全部可选 */
export interface DispAuditListInput {
  refId?: string;
  action?: DispositionAuditAction;
  /** 默认 50，最大 200（越界后端截断） */
  limit?: number;
  /** 默认 0 */
  offset?: number;
}

/* ---------------- 处置（§2.6） ---------------- */

/**
 * 单项能力 reason 集合 — 仅 false 项填说明，true 项省略。
 * 序列化为 camelCase。
 */
export interface DispReasons {
  archive?: string;
  softDelete?: string;
  destroy?: string;
  restoreFromBin?: string;
  /** m7-7.4 · 解除关联 */
  unlink?: string;
}

/**
 * `disp_get_capabilities` 出参 — §2.6。
 * UI 据此渲染三档按钮可用态；false 项必须给出 reason。
 */
export interface DispCapabilities {
  archive: boolean;
  softDelete: boolean;
  destroy: boolean;
  restoreFromBin: boolean;
  /** m7-7.4 · 解除关联（仅 external 为 true） */
  unlink: boolean;
  reason: DispReasons;
}

/** `disp_preview` 出参 — §2.6 */
export interface DispPreview {
  previewId: string;
  target: string;
  isDir: boolean;
  fileCount: number;
  totalBytes: number;
  capability: DispCapabilities;
  warning: string;
}

/** `disp_destroy` 出参 — §2.6 二选一固化：返回被删除的 refId */
export interface DispDestroyResult {
  deletedRefId: string;
}

/** `disp_unlink` 出参 — m7-7.4：返回被删除的 refId */
export interface DispUnlinkResult {
  deletedRefId: string;
}

/* ---------------- 收件箱（§2.7） ---------------- */

/** 收件箱条目状态 — 与 inbox_item.status CHECK 一致 */
export type InboxStatus = "pending" | "snoozed" | "processed" | "ignored" | "stale";

/** 收件箱事件类型 — inbox_item.event_kind */
export type InboxEventKind = "created" | "modified" | "renamed" | "removed";

/** 忽略规则种类 — inbox_ignore.rule.kind */
export type InboxIgnoreRuleKind = "once" | "by_ext" | "by_name" | "by_dir";

/**
 * 收件箱条目 — 详细设计说明书 §2.7 / §3.2 inbox_item 表。
 * 序列化为 camelCase（与后端 serde rename_all 对齐）。
 * discoveredAt / mtime / remindAt 为 Unix 秒（number）。
 */
export interface InboxItem {
  id: string;
  watchDirId?: string | null;
  /** 绝对路径 */
  path: string;
  eventKind?: InboxEventKind | null;
  sizeBytes?: number | null;
  mtime?: number | null;
  /** 小写扩展名，无点 */
  ext?: string | null;
  /** 建议类型（六类型之一；可空表示未识别） */
  suggestedType?: ReferenceType | null;
  status: InboxStatus;
  /** 处理动作快照（assign 时写入）；JSON 字符串 */
  assignJson?: string | null;
  ignoreRuleId?: string | null;
  snoozeNote?: string | null;
  remindAt?: number | null;
  discoveredAt: number;
}

/** 文本预览（契约 §2.7 预览形状，后端 inbox_get 已按此返回） */
export interface InboxPreview {
  kind: "text";
  /** 前 N 行文本 */
  lines: string[];
  /** 是否被截断（实际行数 > N） */
  truncated: boolean;
}

/**
 * 收件箱条目详情 — inbox_get 出参。
 * 在 InboxItem 基础上扩展可选预览与敏感提示。
 * 敏感文件：sensitiveWarning 非空时前端不显示预览（§6.9）。
 */
export interface InboxItemDetail extends InboxItem {
  preview?: InboxPreview | null;
  /** 敏感文件风险提示；非空时前端显示黄色 Alert 且不显示预览 */
  sensitiveWarning?: string | null;
}

/** inbox_stats 出参 — §2.7 */
export interface InboxStats {
  pending: number;
  snoozed: number;
  /** 源文件已不存在的失效条目数（改名/删除自动标记） */
  stale: number;
  lastEventAt?: number | null;
}

/** inbox_list 入参 */
export interface InboxListInput {
  status?: InboxStatus;
  limit?: number;
  offset?: number;
}

/** inbox_snooze 入参 */
export interface InboxSnoozeInput {
  id: string;
  note?: string;
  remindAt?: number;
}

/** inbox_ignore 入参 */
export interface InboxIgnoreInput {
  id: string;
  rule?: {
    kind: InboxIgnoreRuleKind;
    value?: string;
  };
}

/** inbox_assign 处理方式 — §2.7 */
export type InboxAssignMode = "external" | "managed";

/**
 * inbox_assign 入参 — 详细设计说明书 §2.7。
 * - mode=external：仅关联，confirmed 必须为 true（无 plan 阶段）
 * - mode=managed + confirmed=false：拿 ManagedPlan 二次确认
 * - mode=managed + confirmed=true：执行导入并托管
 * targetName 仅在 managed + confirmed=true 阶段生效（缺省用源名）
 */
export interface InboxAssignInput {
  id: string;
  mode: InboxAssignMode;
  spaceId: string;
  collectionId: string;
  type: ReferenceType;
  managedAction?: ManagedAction;
  targetName?: string;
  confirmed: boolean;
}

/**
 * inbox_assign 出参 — §2.7。
 * - external / managed confirmed=true：reference 为落地后的正式引用
 * - managed confirmed=false：managedPlan 非空，用于二次确认
 */
export interface InboxAssignResult {
  inboxItem: InboxItem;
  reference?: Reference | null;
  managedPlan?: ManagedPlan | null;
}

/** inbox_dismiss_stale 入参 — §2.7 */
export interface InboxDismissStaleInput {
  id: string;
}

/* ---------------- 监控目录（M6 §2.8） ---------------- */

/** 监控目录配置 — M6 声明契约 */
export interface WatchDirConfig {
  id: string;
  path: string;
  name: string;
  description?: string;
  /** 监听是否已暂停（1=挂载失败被暂停，0/缺省=正常）；仅出参有意义 */
  paused?: number | null;
}

/** 监控事件 — M6 声明契约 */
export interface WatchEvent {
  type: "created" | "modified" | "renamed" | "removed";
  sourceType: "directory";
  sourcePath: string;
  created: number;
  modified?: number;
}

/* ---------------- 待办（M7-2 · todo） ---------------- */

/** 待办状态 — 与 todo.status CHECK 一致 */
export type TodoStatus = "pending" | "doing" | "done" | "cancelled";

/** 待办优先级：0 普通 / 1 重要 / 2 紧急 */
export type TodoPriority = 0 | 1 | 2;

/**
 * 待办实体 — 任务包 m7-7.2 / 0007_todo.sql todo 表。
 * createdAt / updatedAt / dueAt / doneAt 为 Unix 秒（number）。
 * spaceId 为 null/undefined 表示全局 todo。
 */
export interface Todo {
  id: string;
  title: string;
  note?: string;
  status: TodoStatus;
  spaceId?: string;
  /** m8-8.5 · 资源集级 todo：挂载的资源集 id */
  collectionId?: string;
  priority: TodoPriority;
  dueAt?: number;
  doneAt?: number;
  sortOrder: number;
  createdAt: number;
  updatedAt: number;
}

/** todo_get 出参：Todo + 挂载的资源引用数组 */
export interface TodoWithRefs extends Todo {
  refs: Reference[];
}

/** todo_create 入参 */
export interface TodoCreateInput {
  title: string;
  note?: string;
  spaceId?: string;
  /** m8-8.5 · 挂到指定资源集 */
  collectionId?: string;
  priority?: TodoPriority;
  dueAt?: number;
}

/**
 * todo_update 入参（patch 语义）。
 * 对可空字段（note / spaceId / dueAt）：
 * - 字段缺失 / undefined → 不更新
 * - 显式 null → 置空（写 NULL）
 * - 具体值 → 更新为该值
 */
export interface TodoPatch {
  title?: string;
  note?: string | null;
  spaceId?: string | null;
  /** m8-8.5 · 资源集归属：null = 解除挂载 */
  collectionId?: string | null;
  priority?: TodoPriority;
  dueAt?: number | null;
  sortOrder?: number;
}

/** todo_list 入参 */
export interface TodoListInput {
  /** 缺省 = 全部；传 "global" 仅返回全局 todo（space_id IS NULL） */
  spaceId?: string;
  /** m8-8.5 · 按资源集过滤（与 spaceId 可叠加） */
  collectionId?: string;
  /** 缺省 = 仅 pending+doing；传 "all" 返回全部；传具体状态值按该状态过滤 */
  status?: TodoStatus | "all";
  /** 兼容参数：true 等价于 status="all" */
  includeDone?: boolean;
}

/** todo_set_status 入参 */
export interface TodoSetStatusInput {
  id: string;
  status: TodoStatus;
}

/** todo_link_ref / todo_unlink_ref 入参 */
export interface TodoRefLinkInput {
  todoId: string;
  refId: string;
}

/* ---------------- 资源打开/操作（M7-1） ---------------- *//**
 * `ref_open` 出参 — §2.5。
 * strategy 标识实际命中的打开策略：
 * - `system_default`：系统默认程序
 * - `app`           ：类型默认程序（settings.default_app_{type}）
 * - `custom`        ：本次调用显式 appOverride
 */
export interface OpenResult {
  opened: boolean;
  strategy: "system_default" | "app" | "custom";
}

/**
 * `ref_log_access` 入参 action — 与 0006_ref_access_log.sql CHECK 约束一致。
 * - `open`      ：默认打开
 * - `reveal`    ：在文件管理器中显示
 * - `copy_path` ：复制路径到剪贴板
 * - `open_with` ：用其他程序打开
 */
export type RefAccessAction = "open" | "reveal" | "copy_path" | "open_with";

/* ---------------- Dashboard（M7-3） ---------------- */

/**
 * `ref_recent_access` 出参条目 — 任务包 m7-7.3。
 * 按 refId 去重，每个 ref 仅保留最近一次 access_log。
 */
export interface RecentRef {
  refId: string;
  refName: string;
  /** 引用类型：'code' | 'document' | ... */
  refType: ReferenceType | string;
  /** 最近一次动作 */
  lastAction: RefAccessAction;
  /** Unix 秒 */
  lastAt: number;
  /** 透传 resource_reference.locator_json（前端展示路径用） */
  locatorJson: string;
}

/** `settings_get_default_home` / `settings_set_default_home` 出参 */
export interface DefaultHomeConfig {
  /** "dashboard" | "spaces" */
  home: DefaultHome;
}

export type DefaultHome = "dashboard" | "spaces";

/** `settings_get_user_name` 出参（M7-3 Dashboard 问候语） */
export interface UserNameConfig {
  /** 用户名；未设置时字段缺失（前端显示"朋友"） */
  userName?: string;
}

/** `settings_get/set_launch_at_login` 出参（开机自启开关） */
export interface LaunchAtLoginConfig {
  /** 是否开机自启；未设置时后端默认 true */
  enabled: boolean;
}

/* ---------------- 导入撤销（m4-4.9 · ref_undo_import） ---------------- */

/**
 * `ref_undo_import(confirmed=false)` 出参 — UndoPlan。
 * 不写文件不改库；前端据此渲染确认框。
 * blockers 非空时 canUndo=false，确认按钮应禁用。
 */
export interface UndoPlan {
  refId: string;
  refName: string;
  /** "copy" | "move" */
  managedAction: ManagedAction;
  /** 当前目标绝对路径（locator_json.path） */
  currentPath: string;
  /** 原始源绝对路径；M3 老数据为 null */
  originalSource: string | null;
  /** 是否可撤销：blockers 为空时 true */
  canUndo: boolean;
  /** 阻塞原因列表（中文）；空数组表示可撤销 */
  blockers: string[];
}
/* ---------------- 内嵌终端（M7-4 · terminal_*） ---------------- */

/**
 * 终端事件（与后端 `TerminalEvent` 对齐，snake_case tag）。
 * 后端通过 `tauri::ipc::Channel<TerminalEvent>` 持续推送。
 */
export type TerminalEvent =
  | { kind: "data"; data: string }
  | { kind: "exited"; code: number | null };

/** `terminal_create` 入参。 */
export interface TerminalCreateInput {
  spaceId?: string;
  cwd?: string;
  cols: number;
  rows: number;
}

/** `terminal_create` 出参。 */
export interface TerminalCreateOutput {
  sessionId: string;
}

/** `terminal_list` 出参条目。 */
export interface TerminalListItem {
  sessionId: string;
  spaceId?: string;
  cwd: string;
  shell: string;
}
