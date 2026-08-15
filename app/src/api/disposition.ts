import { invoke } from "@tauri-apps/api/core";
import type { DispositionAudit, DispAuditListInput } from "./types";
import { mockDispositionApi } from "./mock";

const MOCK = import.meta.env.VITE_MOCK_API === "true";

/**
 * 处置审计 API — 契约见详细设计说明书 §2.6 disp_audit_list。
 * 后端未就绪时可通过 VITE_MOCK_API=true 切换到 mock 数据。
 *
 * 关键约束（§6.7）：`disposition_audit` 无外键，销毁后审计独立存活；
 * 前端展示时 `refName` / `locatorSnapshot` 直接来自写入时的快照，
 * 不依赖 `resource_reference` 当前行。
 */

export async function dispAuditList(input: DispAuditListInput): Promise<DispositionAudit[]> {
  if (MOCK) return mockDispositionApi.disp_audit_list(input);
  return invoke<DispositionAudit[]>("disp_audit_list", { ...input });
}
