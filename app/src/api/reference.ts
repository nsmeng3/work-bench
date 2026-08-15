import { invoke } from "@tauri-apps/api/core";
import type { Reference, RefUpdateInput } from "./types";
import { mockReferenceApi } from "./mock";

const MOCK = import.meta.env.VITE_MOCK_API === "true";

/**
 * 资源引用 API — 契约见详细设计说明书 §2.5。
 * `ref_update` 仅允许修改管理属性（name/description/tags/lifecycle/confidentiality/indexed），
 * `type` / `locator` / `hosting` 不可改。
 * 后端未就绪时可通过 VITE_MOCK_API=true 切换到 mock 数据。
 */

export async function refUpdate(input: RefUpdateInput): Promise<Reference> {
  if (MOCK) return mockReferenceApi.ref_update(input);
  return invoke<Reference>("ref_update", { ...input });
}
