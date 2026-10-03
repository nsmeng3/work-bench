import { invoke } from "@tauri-apps/api/core";
import type {
  Collection,
  CollectionCreateInput,
  CollectionUpdateInput,
  CollectionIdInput,
  CollectionListInput,
  CollectionDetail,
  RecentCollection,
} from "./types";
import { mockCollectionApi } from "./mock";

const MOCK = import.meta.env.VITE_MOCK_API === "true";

/**
 * 资源集管理 API — 契约见详细设计说明书 §2.4。
 * 后端未就绪时可通过 VITE_MOCK_API=true 切换到 mock 数据。
 */

export async function collectionList(input: CollectionListInput): Promise<Collection[]> {
  if (MOCK) return mockCollectionApi.collection_list(input);
  return invoke<Collection[]>("collection_list", { ...input });
}

export async function collectionCreate(input: CollectionCreateInput): Promise<Collection> {
  if (MOCK) return mockCollectionApi.collection_create(input);
  return invoke<Collection>("collection_create", { ...input });
}

export async function collectionUpdate(input: CollectionUpdateInput): Promise<Collection> {
  if (MOCK) return mockCollectionApi.collection_update(input);
  return invoke<Collection>("collection_update", { ...input });
}

export async function collectionArchive(input: CollectionIdInput): Promise<Collection> {
  if (MOCK) return mockCollectionApi.collection_archive(input);
  return invoke<Collection>("collection_archive", { ...input });
}

export async function collectionRestore(input: CollectionIdInput): Promise<Collection> {
  if (MOCK) return mockCollectionApi.collection_restore(input);
  return invoke<Collection>("collection_restore", { ...input });
}

export async function collectionGet(input: CollectionIdInput): Promise<CollectionDetail> {
  if (MOCK) return mockCollectionApi.collection_get(input);
  return invoke<CollectionDetail>("collection_get", { ...input });
}

/**
 * `collection_recent_access { limit? }` → `RecentCollection[]`。
 * 由 ref_access_log 聚合：打开过资源即算访问了其所属资源集，按最近访问倒序。
 */
export async function collectionRecentAccess(limit?: number): Promise<RecentCollection[]> {
  if (MOCK) return mockCollectionApi.collection_recent_access(limit);
  return invoke<RecentCollection[]>("collection_recent_access", { limit });
}
