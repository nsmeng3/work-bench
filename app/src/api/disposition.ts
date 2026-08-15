import { invoke } from "@tauri-apps/api/core";
import type { DispositionAudit, DispAuditListInput } from "./types";
import type {
  DispCapabilities,
  DispDestroyResult,
  DispPreview,
  Reference,
  UndoPlan,
} from "./types";
import { mockDispositionApi } from "./mock";

const MOCK = import.meta.env.VITE_MOCK_API === "true";

/**
 * 处置 API — 契约见详细设计说明书 §2.6。
 * 后端 4.1-4.5 已落地 disp_get_capabilities / disp_archive / disp_unarchive /
 * disp_preview / disp_destroy / disp_soft_delete；disp_audit_list 由 4.6 提供。
 * 后端未就绪时可通过 VITE_MOCK_API=true 切换到 mock 数据。
 *
 * 审计关键约束（§6.7）：`disposition_audit` 无外键，销毁后审计独立存活；
 * 前端展示时 `refName` / `locatorSnapshot` 直接来自写入时的快照，
 * 不依赖 `resource_reference` 当前行。
 */

/** `disp_get_capabilities { refId }` → `DispCapabilities` */
export async function dispGetCapabilities(refId: string): Promise<DispCapabilities> {
  if (MOCK) return mockDispositionApi.disp_get_capabilities(refId);
  return invoke<DispCapabilities>("disp_get_capabilities", { refId });
}

/** `disp_archive { refId }` → `Reference`（disposition: none → archived） */
export async function dispArchive(refId: string): Promise<Reference> {
  if (MOCK) return mockDispositionApi.disp_archive(refId);
  return invoke<Reference>("disp_archive", { refId });
}

/** `disp_unarchive { refId }` → `Reference`（disposition: archived → none） */
export async function dispUnarchive(refId: string): Promise<Reference> {
  if (MOCK) return mockDispositionApi.disp_unarchive(refId);
  return invoke<Reference>("disp_unarchive", { refId });
}

/** `disp_soft_delete { refId, confirmed: true }` → `Reference`（disposition: → deleted） */
export async function dispSoftDelete(refId: string): Promise<Reference> {
  if (MOCK) return mockDispositionApi.disp_soft_delete(refId);
  return invoke<Reference>("disp_soft_delete", { refId, confirmed: true });
}

/**
 * `disp_destroy { refId, confirmText, confirmed: true }` → `DispDestroyResult`。
 * confirmText 必须与资源当前名称完全一致（逐字符），否则 `COMMON_CONFIRM_REQUIRED`。
 */
export async function dispDestroy(refId: string, confirmText: string): Promise<DispDestroyResult> {
  if (MOCK) return mockDispositionApi.disp_destroy(refId, confirmText);
  return invoke<DispDestroyResult>("disp_destroy", {
    refId,
    confirmText,
    confirmed: true,
  });
}

/** `disp_preview { refId }` → `DispPreview`（删除/销毁确认框的统计数据） */
export async function dispPreview(refId: string): Promise<DispPreview> {
  if (MOCK) return mockDispositionApi.disp_preview(refId);
  return invoke<DispPreview>("disp_preview", { refId });
}

/** `disp_audit_list { refId?, action?, limit?, offset? }` → `DispositionAudit[]` */
export async function dispAuditList(input: DispAuditListInput): Promise<DispositionAudit[]> {
  if (MOCK) return mockDispositionApi.disp_audit_list(input);
  return invoke<DispositionAudit[]>("disp_audit_list", { ...input });
}

/**
 * `ref_undo_import { refId, confirmed }` → `UndoPlan | null`（m4-4.9）。
 *
 * - `confirmed=false` → 返回 `UndoPlan`（不写文件不改库）
 * - `confirmed=true`  → 返回 `null`（执行撤销：删目标/移回 + 删引用 + 写审计）
 *
 * 前端使用模式：先调 plan 拿 UndoPlan 渲染确认框，用户确认后再调 confirmed=true。
 */
export async function refUndoImport(refId: string, confirmed: boolean): Promise<UndoPlan | null> {
  if (MOCK) return mockDispositionApi.ref_undo_import(refId, confirmed);
  return invoke<UndoPlan | null>("ref_undo_import", { refId, confirmed });
}
