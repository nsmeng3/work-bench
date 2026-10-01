import { invoke } from "@tauri-apps/api/core";
import type {
  ManagedPlan,
  OpenResult,
  RefAccessAction,
  RefCreateExternalInput,
  RefCreateManagedInput,
  Reference,
  RefUpdateInput,
} from "./types";
import { mockReferenceApi } from "./mock";

const MOCK = import.meta.env.VITE_MOCK_API === "true";

/**
 * 资源引用 API — 契约见详细设计说明书 §2.5。
 * 后端未就绪时可通过 VITE_MOCK_API=true 切换到 mock 数据。
 *
 * 关键约束（§6.3）：`ref_create_external` 仅做登记，绝不复制/移动/写入源文件。
 * `ref_update` 仅允许修改管理属性（name/description/tags/lifecycle/confidentiality/indexed），
 * `type` / `locator` / `hosting` 不可改。
 */

export async function refCreateExternal(input: RefCreateExternalInput): Promise<Reference> {
  if (MOCK) return mockReferenceApi.ref_create_external(input);
  return invoke<Reference>("ref_create_external", { ...input });
}

export async function refUpdate(input: RefUpdateInput): Promise<Reference> {
  if (MOCK) return mockReferenceApi.ref_update(input);
  return invoke<Reference>("ref_update", { ...input });
}

/**
 * 导入并托管 — §2.5 ref_create_managed（两阶段）。
 *
 * - confirmed=false：返回 ManagedPlan，后端不做任何写操作；
 * - confirmed=true：执行 copy/move，返回 Reference。
 *
 * 前端通过 `ManagedPlan | Reference` 联合类型承载两种返回，
 * 调用方依据 `confirmed` 入参做类型窄化（见 ReferenceManagedDialog）。
 *
 * 3.7 hook：confirmed=true 阶段调用前，调用方应生成临时 refId（UUID）并通过
 * onRefIdReady 回调暴露，供 3.7 订阅 `managed_progress` 事件做进度展示。
 * 本任务不实现进度 UI，仅在 ReferenceManagedDialog 中预留该 hook 点。
 */
export async function refCreateManaged(input: RefCreateManagedInput & { confirmed: false }): Promise<ManagedPlan>;
export async function refCreateManaged(input: RefCreateManagedInput & { confirmed: true }): Promise<Reference>;
export async function refCreateManaged(input: RefCreateManagedInput): Promise<ManagedPlan | Reference> {
  if (MOCK) {
    return input.confirmed
      ? mockReferenceApi.ref_create_managed_confirmed(input)
      : mockReferenceApi.ref_create_managed_plan(input);
  }
  return invoke<ManagedPlan | Reference>("ref_create_managed", { ...input });
}

/* ---------------- M7-1 · 资源打开/操作 ---------------- */

/**
 * `ref_open { id, appOverride? }` → `OpenResult`。
 *
 * 三级策略（§5.3）：appOverride > 类型默认 > 系统默认。
 * 调用方通常使用 `refOpen(refId)` 或 `refOpenWith(refId, appPath)`。
 */
export async function refOpen(refId: string, appOverride?: string): Promise<OpenResult> {
  if (MOCK) return mockReferenceApi.ref_open(refId, appOverride);
  return invoke<OpenResult>("ref_open", { id: refId, appOverride });
}

/** `ref_reveal_in_finder { id }` → void。在系统文件管理器中显示并选中。 */
export async function refRevealInFinder(refId: string): Promise<void> {
  if (MOCK) return mockReferenceApi.ref_reveal_in_finder(refId);
  return invoke<void>("ref_reveal_in_finder", { id: refId });
}

/**
 * 复制引用路径到剪贴板 — 纯前端动作，无需后端命令。
 * 仅在 locator.kind === "path" 时可用；其他形态抛 COMMON_INVALID_PARAM。
 *
 * 注意：调用方应在成功后自行调 `refLogAccess(refId, "copy_path")` 埋点。
 */
export async function refCopyPath(ref: Reference): Promise<void> {
  if (ref.locator.kind !== "path") {
    throw {
      code: "COMMON_INVALID_PARAM",
      message: `引用 ${ref.name} 的定位非 path 形态，无法复制路径`,
      retryable: false,
    };
  }
  await navigator.clipboard.writeText(ref.locator.path);
}

/**
 * `ref_open` 带 appOverride — 等价于 `refOpen(refId, appPath)`。
 * 单独导出仅为语义清晰（UI 上"用其他程序打开…"）。
 */
export async function refOpenWith(refId: string, appPath: string): Promise<OpenResult> {
  return refOpen(refId, appPath);
}

/**
 * `ref_log_access { refId, action }` → void。
 *
 * 埋点命令：前端在 open/reveal/copy_path/open_with 成功后调用。
 * 失败语义：调用方应 `await` 但捕获错误仅 `console.warn`，不影响主流程。
 */
export async function refLogAccess(refId: string, action: RefAccessAction): Promise<void> {
  if (MOCK) return mockReferenceApi.ref_log_access(refId, action);
  return invoke<void>("ref_log_access", { refId, action });
}

/**
 * 埋点安全调用 — 吞掉错误仅 console.warn。
 * 供 UI 层"成功后埋点"使用，避免埋点失败影响主流程。
 */
export async function refLogAccessSafe(refId: string, action: RefAccessAction): Promise<void> {
  try {
    await refLogAccess(refId, action);
  } catch (err) {
    // 埋点失败可接受，不打断用户操作
    console.warn("[ref_log_access] 埋点失败", action, refId, err);
  }
}
