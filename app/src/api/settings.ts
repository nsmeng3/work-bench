import { invoke } from "@tauri-apps/api/core";
import type {
  RootDirStatus,
  InitRootDirInput,
  InitRootDirResult,
  StorageSourceInfo,
  StorageSourceUpdateInput,
  DefaultAppConfig,
  DefaultAppSetInput,
  SettingsChangeRootDirInput,
  ChangeRootResult,
} from "./types";
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

/**
 * 列出存储源 — §2.8 settings_list_sources。
 * 第一阶段仅返回默认 LocalFsSource。
 */
export async function settingsListSources(): Promise<StorageSourceInfo[]> {
  if (MOCK) return mockSettingsApi.settings_list_sources();
  return invoke<StorageSourceInfo[]>("settings_list_sources");
}

/**
 * 更新存储源 — §2.8 settings_update_source。
 * 第一阶段仅允许修改名称。
 */
export async function settingsUpdateSource(
  input: StorageSourceUpdateInput,
): Promise<StorageSourceInfo> {
  if (MOCK) return mockSettingsApi.settings_update_source(input);
  return invoke<StorageSourceInfo>("settings_update_source", { ...input });
}

/**
 * 获取某类型的默认查看程序配置 — §2.8 settings_get_default_app。
 */
export async function settingsGetDefaultApp(
  type: string,
): Promise<DefaultAppConfig> {
  if (MOCK) return mockSettingsApi.settings_get_default_app(type);
  return invoke<DefaultAppConfig>("settings_get_default_app", { type });
}

/**
 * 设置某类型的默认查看程序配置 — §2.8 settings_set_default_app。
 */
export async function settingsSetDefaultApp(
  input: DefaultAppSetInput,
): Promise<DefaultAppConfig> {
  if (MOCK) return mockSettingsApi.settings_set_default_app(input);
  return invoke<DefaultAppConfig>("settings_set_default_app", { ...input });
}

/**
 * 修改资源根目录 — §2.8 settings_change_root_dir（两阶段）。
 *
 * - confirmed=false → MigrationPlan（不写文件不改库，前端据此渲染确认页）
 * - confirmed=true && strategy="future_only" → FutureOnlyResult
 * - confirmed=true && strategy="migrate" → MigrationResult
 *
 * 出参为 untagged 联合，请用 isMigrationPlan / isMigrationResult /
 * isFutureOnlyResult 类型守卫鉴别。
 */
export async function settingsChangeRootDir(
  input: SettingsChangeRootDirInput,
): Promise<ChangeRootResult> {
  if (MOCK) return mockSettingsApi.settings_change_root_dir(input);
  return invoke<ChangeRootResult>("settings_change_root_dir", { ...input });
}
