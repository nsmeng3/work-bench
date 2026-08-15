import { invoke } from "@tauri-apps/api/core";
import type { Reference, RefCreateExternalInput } from "./types";
import { mockReferenceApi } from "./mock";

const MOCK = import.meta.env.VITE_MOCK_API === "true";

/**
 * 资源引用 API — 契约见详细设计说明书 §2.5。
 * 后端未就绪时可通过 VITE_MOCK_API=true 切换到 mock 数据。
 *
 * 关键约束（§6.3）：`ref_create_external` 仅做登记，绝不复制/移动/写入源文件。
 */

export async function refCreateExternal(input: RefCreateExternalInput): Promise<Reference> {
  if (MOCK) return mockReferenceApi.ref_create_external(input);
  return invoke<Reference>("ref_create_external", { ...input });
}
