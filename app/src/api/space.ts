import { invoke } from "@tauri-apps/api/core";
import type { Space, SpaceCreateInput, SpaceUpdateInput, SpaceIdInput, SpaceListInput } from "./types";
import { mockSpaceApi } from "./mock";

const MOCK = import.meta.env.VITE_MOCK_API === "true";

/**
 * 空间管理 API — 契约见详细设计说明书 §2.3。
 * 后端未就绪时可通过 VITE_MOCK_API=true 切换到 mock 数据。
 */

export async function spaceList(input: SpaceListInput = {}): Promise<Space[]> {
  if (MOCK) return mockSpaceApi.space_list(input);
  return invoke<Space[]>("space_list", { ...input });
}

export async function spaceCreate(input: SpaceCreateInput): Promise<Space> {
  if (MOCK) return mockSpaceApi.space_create(input);
  return invoke<Space>("space_create", { ...input });
}

export async function spaceUpdate(input: SpaceUpdateInput): Promise<Space> {
  if (MOCK) return mockSpaceApi.space_update(input);
  return invoke<Space>("space_update", { ...input });
}

export async function spaceArchive(input: SpaceIdInput): Promise<Space> {
  if (MOCK) return mockSpaceApi.space_archive(input);
  return invoke<Space>("space_archive", { ...input });
}

export async function spaceRestore(input: SpaceIdInput): Promise<Space> {
  if (MOCK) return mockSpaceApi.space_restore(input);
  return invoke<Space>("space_restore", { ...input });
}
