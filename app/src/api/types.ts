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
 */
export interface Space {
  id: string;
  name: string;
  description?: string;
  color?: string;
  icon?: string;
  status: "active" | "archived";
  createdAt: string;
  updatedAt: string;
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
