import { invoke } from "@tauri-apps/api/core";
import type {
  ManagedPlan,
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
