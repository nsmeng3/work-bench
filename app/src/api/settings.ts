import { invoke } from "@tauri-apps/api/core";
import type { RootDirStatus, InitRootDirInput, InitRootDirResult } from "./types";
import { mockSettingsApi } from "./mock";

const MOCK = import.meta.env.VITE_MOCK_API === "true";

/**
 * 设置中心 API — 契约见详细设计说明书 §2.8。
 * 后端未就绪时可通过 VITE_MOCK_API=true 切换到 mock 数据。
 */

export async function settingsGetRootDir(): Promise<RootDirStatus> {
  if (MOCK) return mockSettingsApi.settings_get_root_dir();
  return invoke<RootDirStatus>("settings_get_root_dir");
}

export async function settingsInitRootDir(
  input: InitRootDirInput,
): Promise<InitRootDirResult> {
  if (MOCK) return mockSettingsApi.settings_init_root_dir(input);
  return invoke<InitRootDirResult>("settings_init_root_dir", { ...input });
}
