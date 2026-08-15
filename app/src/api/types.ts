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

/* ---------------- 资源引用（Reference） ---------------- */

/** 引用类型 — 详细设计说明书 §2.5 / §3.2 resource_reference 表 */
export type ReferenceType = "code" | "document" | "data" | "artifact" | "tool" | "media";

/** 引用健康度 — §2.4 collection_get.referencesByType.*.health */
export type ReferenceHealth = "ok" | "missing" | "unknown";

/** 引用生命周期 */
export type ReferenceLifecycle = "active" | "staged" | "delivered" | "archived";

/** 引用保密级别 */
export type ReferenceConfidentiality =
  | "public"
  | "internal"
  | "customer_restricted"
  | "sensitive";

/** 引用托管方式 */
export type ReferenceHosting = "external" | "managed";

/** 引用处置状态 */
export type ReferenceDisposition = "none" | "archived" | "deleted";

/** 引用定位（locator）— §3.2 resource_reference.locator_json */
export type ReferenceLocator =
  | { kind: "path"; path: string }
  | { kind: "repo"; local: string; remote?: string; defaultBranch?: string }
  | { kind: "cloud"; provider: string; objectId: string };

/**
 * 资源引用实体 — §2.5 / §3.2 resource_reference 表。
 * createdAt / updatedAt 为 Unix 秒（number）。
 */
export interface Reference {
  id: string;
  collectionId: string;
  sourceId: string;
  name: string;
  type: ReferenceType;
  hosting: ReferenceHosting;
  locator: ReferenceLocator;
  description?: string;
  tags?: string[];
  lifecycle: ReferenceLifecycle;
  confidentiality: ReferenceConfidentiality;
  indexed: boolean;
  disposition: ReferenceDisposition;
  createdAt: number;
  updatedAt: number;
}

/** collection_get.referencesByType 中每个引用条目携帯健康度 */
export interface ReferenceWithHealth {
  ref: Reference;
  health: ReferenceHealth;
}

/**
 * ref_update 入参 — §2.5：仅允许修改管理属性；
 * type / locator / hosting 不可改，故不在此出现。
 * 所有字段可选，仅传入需要修改的字段；tags 为全量替换语义。
 */
export interface RefUpdateInput {
  id: string;
  name?: string;
  description?: string;
  tags?: string[];
  lifecycle?: ReferenceLifecycle;
  confidentiality?: ReferenceConfidentiality;
  indexed?: boolean;
}

/** collection_get 出参 — §2.4 CollectionDetail */
export interface CollectionDetail {
  id: string;
  spaceId: string;
  name: string;
  summary?: string;
  tags?: string[];
  status: "active" | "archived";
  createdAt: number;
  updatedAt: number;
  /** 六类型分组；固定键序 code/document/data/artifact/tool/media */
  referencesByType: Record<ReferenceType, ReferenceWithHealth[]>;
}
