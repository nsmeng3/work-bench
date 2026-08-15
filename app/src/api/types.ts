/**
 * 统一错误结构 — 详细设计说明书 §2.2
 */
export interface ApiError {
  code: string;
  message: string;
  details?: Record<string, unknown>;
  retryable: boolean;
}

/**
 * 空间实体 — 详细设计说明书 §2.3 / §3.2 space 表
 * createdAt / updatedAt 为 Unix 秒（number）。
 */
export interface Space {
  id: string;
  name: string;
  description?: string;
  color?: string;
  icon?: string;
  status: "active" | "archived";
  createdAt: number;
  updatedAt: number;
}

/** space_create 入参 */
export interface SpaceCreateInput {
  name: string;
  description?: string;
  color?: string;
  icon?: string;
}

/** space_update 入参 */
export interface SpaceUpdateInput {
  id: string;
  name?: string;
  description?: string;
  color?: string;
  icon?: string;
}

/** space_archive / space_restore 入参 */
export interface SpaceIdInput {
  id: string;
}

/** space_list 入参 */
export interface SpaceListInput {
  status?: "active" | "archived" | "all";
}

/**
 * 资源集实体 — 详细设计说明书 §2.4 / §3.2 collection 表。
 * createdAt / updatedAt 为 Unix 秒（number）。
 */
export interface Collection {
  id: string;
  spaceId: string;
  name: string;
  summary?: string;
  tags?: string[];
  status: "active" | "archived";
  createdAt: number;
  updatedAt: number;
}

/** collection_create 入参 */
export interface CollectionCreateInput {
  spaceId: string;
  name: string;
  summary?: string;
  tags?: string[];
}

/** collection_update 入参 */
export interface CollectionUpdateInput {
  id: string;
  name?: string;
  summary?: string;
  tags?: string[];
}

/** collection_archive / collection_restore / collection_get 入参 */
export interface CollectionIdInput {
  id: string;
}

/** collection_list 入参 */
export interface CollectionListInput {
  spaceId: string;
  status?: "active" | "archived" | "all";
}
