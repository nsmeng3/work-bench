import { invoke } from "@tauri-apps/api/core";
import type {
  DispCapabilities,
  DispDestroyResult,
  DispPreview,
  Reference,
} from "./types";
import { mockDispositionApi } from "./mock";

const MOCK = import.meta.env.VITE_MOCK_API === "true";

/**
 * 处置 API — 契约见详细设计说明书 §2.6。
 * 后端 4.1-4.5 已落地 disp_get_capabilities / disp_archive / disp_unarchive /
 * disp_preview / disp_destroy；disp_soft_delete 由 4.4 提供，若后端未合并
 * 可通过 VITE_MOCK_API=true 切换到 mock。
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
