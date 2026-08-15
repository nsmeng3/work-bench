import type {
  Space,
  SpaceCreateInput,
  SpaceUpdateInput,
  SpaceIdInput,
  SpaceListInput,
  Collection,
  CollectionCreateInput,
  CollectionUpdateInput,
  CollectionIdInput,
  CollectionListInput,
  CollectionDetail,
  Reference,
  ReferenceType,
  ReferenceWithHealth,
  RefCreateExternalInput,
} from "./types";

/**
 * Mock 数据 — 形状与契约逐字段一致（§2.3 / §3.2 space 表）。
 * 仅在 VITE_MOCK_API 开启时使用，默认关闭。
 * createdAt / updatedAt 使用 Unix 秒。
 */

let mockSeq = 100;

function unixNow(): number {
  return Math.floor(Date.now() / 1000);
}

function makeId(): string {
  return `mock-space-${++mockSeq}`;
}

const seedSpaces: Space[] = [
  {
    id: "mock-space-1",
    name: "工作",
    description: "工作相关项目",
    color: "#4A90D9",
    icon: "briefcase",
    status: "active",
    createdAt: 1754038800, // 2026-08-01T09:00:00Z
    updatedAt: 1754038800,
  },
  {
    id: "mock-space-2",
    name: "生活",
    description: "个人生活资源",
    color: "#7BC47F",
    icon: "home",
    status: "active",
    createdAt: 1754038860, // 2026-08-01T09:01:00Z
    updatedAt: 1754038860,
  },
];

let store: Space[] = [...seedSpaces];

export const mockSpaceApi = {
  space_list(input: SpaceListInput): Space[] {
    const status = input.status ?? "active";
    if (status === "all") return [...store];
    return store.filter((s) => s.status === status);
  },

  space_create(input: SpaceCreateInput): Space {
    const now = unixNow();
    const space: Space = {
      id: makeId(),
      name: input.name,
      description: input.description,
      color: input.color,
      icon: input.icon,
      status: "active",
      createdAt: now,
      updatedAt: now,
    };
    store.push(space);
    return space;
  },

  space_update(input: SpaceUpdateInput): Space {
    const idx = store.findIndex((s) => s.id === input.id);
    if (idx === -1) throw { code: "COMMON_NOT_FOUND", message: "空间不存在", retryable: false };
    const s = store[idx];
    if (s.status === "archived") throw { code: "COMMON_CONFLICT", message: "已归档空间不可编辑", retryable: false };
    const updated: Space = {
      ...s,
      name: input.name ?? s.name,
      description: input.description ?? s.description,
      color: input.color ?? s.color,
      icon: input.icon ?? s.icon,
      updatedAt: unixNow(),
    };
    store[idx] = updated;
    return updated;
  },

  space_archive(input: SpaceIdInput): Space {
    const idx = store.findIndex((s) => s.id === input.id);
    if (idx === -1) throw { code: "COMMON_NOT_FOUND", message: "空间不存在", retryable: false };
    const s = store[idx];
    if (s.status === "archived") throw { code: "COMMON_CONFLICT", message: "空间已归档", retryable: false };
    const updated: Space = { ...s, status: "archived", updatedAt: unixNow() };
    store[idx] = updated;
    return updated;
  },

  space_restore(input: SpaceIdInput): Space {
    const idx = store.findIndex((s) => s.id === input.id);
    if (idx === -1) throw { code: "COMMON_NOT_FOUND", message: "空间不存在", retryable: false };
    const updated: Space = { ...store[idx], status: "active", updatedAt: unixNow() };
    store[idx] = updated;
    return updated;
  },
};

/* ---------------- 资源集 mock ---------------- */

let mockCollectionSeq = 100;

function makeCollectionId(): string {
  return `mock-collection-${++mockCollectionSeq}`;
}

const seedCollections: Collection[] = [
  {
    id: "mock-collection-1",
    spaceId: "mock-space-1",
    name: "工作台前端",
    summary: "资源管理工作台前端代码与文档",
    tags: ["code", "frontend"],
    status: "active",
    createdAt: 1754038920,
    updatedAt: 1754038920,
  },
  {
    id: "mock-collection-2",
    spaceId: "mock-space-1",
    name: "设计文档",
    summary: "概要/详细设计说明书",
    tags: ["doc"],
    status: "active",
    createdAt: 1754038980,
    updatedAt: 1754038980,
  },
];

let collectionStore: Collection[] = [...seedCollections];

export const mockCollectionApi = {
  collection_list(input: CollectionListInput): Collection[] {
    const status = input.status ?? "active";
    const inSpace = collectionStore.filter((c) => c.spaceId === input.spaceId);
    if (status === "all") return [...inSpace];
    return inSpace.filter((c) => c.status === status);
  },

  collection_create(input: CollectionCreateInput): Collection {
    const now = unixNow();
    const collection: Collection = {
      id: makeCollectionId(),
      spaceId: input.spaceId,
      name: input.name,
      summary: input.summary,
      tags: input.tags,
      status: "active",
      createdAt: now,
      updatedAt: now,
    };
    collectionStore.push(collection);
    return collection;
  },

  collection_update(input: CollectionUpdateInput): Collection {
    const idx = collectionStore.findIndex((c) => c.id === input.id);
    if (idx === -1) throw { code: "COMMON_NOT_FOUND", message: "资源集不存在", retryable: false };
    const c = collectionStore[idx];
    if (c.status === "archived")
      throw { code: "COMMON_CONFLICT", message: "已归档资源集不可编辑", retryable: false };
    const updated: Collection = {
      ...c,
      name: input.name ?? c.name,
      summary: input.summary ?? c.summary,
      tags: input.tags ?? c.tags,
      updatedAt: unixNow(),
    };
    collectionStore[idx] = updated;
    return updated;
  },

  collection_archive(input: CollectionIdInput): Collection {
    const idx = collectionStore.findIndex((c) => c.id === input.id);
    if (idx === -1) throw { code: "COMMON_NOT_FOUND", message: "资源集不存在", retryable: false };
    const c = collectionStore[idx];
    if (c.status === "archived")
      throw { code: "COMMON_CONFLICT", message: "资源集已归档", retryable: false };
    const updated: Collection = { ...c, status: "archived", updatedAt: unixNow() };
    collectionStore[idx] = updated;
    return updated;
  },

  collection_restore(input: CollectionIdInput): Collection {
    const idx = collectionStore.findIndex((c) => c.id === input.id);
    if (idx === -1) throw { code: "COMMON_NOT_FOUND", message: "资源集不存在", retryable: false };
    const updated: Collection = { ...collectionStore[idx], status: "active", updatedAt: unixNow() };
    collectionStore[idx] = updated;
    return updated;
  },

  collection_get(input: CollectionIdInput): CollectionDetail {
    const c = collectionStore.find((x) => x.id === input.id);
    if (!c) throw { code: "COMMON_NOT_FOUND", message: "资源集不存在", retryable: false };
    return {
      ...c,
      referencesByType: buildReferencesByType(c.id),
    };
  },
};

/* ---------------- 引用 mock（仅供 collection_get 使用） ---------------- */

const ALL_REFERENCE_TYPES: ReferenceType[] = [
  "code",
  "document",
  "data",
  "artifact",
  "tool",
  "media",
];

function makeRef(partial: Partial<Reference> & Pick<Reference, "id" | "collectionId" | "name" | "type">): Reference {
  const now = 1754039000;
  return {
    sourceId: "mock-source-local",
    hosting: "external",
    locator: { kind: "path", path: `/abs/${partial.type}/${partial.id}` },
    lifecycle: "active",
    confidentiality: "internal",
    indexed: true,
    disposition: "none",
    createdAt: now,
    updatedAt: now,
    ...partial,
  };
}

/** 种子引用：覆盖六类型中的 code/document/data 三类有数据，其余为空；三种 health 各至少一条 */
let seedReferences: ReferenceWithHealth[] = [
  // code
  {
    ref: makeRef({
      id: "mock-ref-code-1",
      collectionId: "mock-collection-1",
      name: "workbench-frontend",
      type: "code",
      description: "工作台前端代码仓",
      tags: ["frontend", "tauri"],
      locator: { kind: "path", path: "/Users/demo/code/workbench/app" },
    }),
    health: "ok",
  },
  {
    ref: makeRef({
      id: "mock-ref-code-2",
      collectionId: "mock-collection-1",
      name: "legacy-scripts",
      type: "code",
      lifecycle: "archived",
      locator: { kind: "path", path: "/Users/demo/code/legacy/scripts" },
    }),
    health: "missing",
  },
  // document
  {
    ref: makeRef({
      id: "mock-ref-doc-1",
      collectionId: "mock-collection-1",
      name: "详细设计说明书",
      type: "document",
      confidentiality: "internal",
      locator: { kind: "path", path: "/Users/demo/docs/详细设计.md" },
    }),
    health: "ok",
  },
  {
    ref: makeRef({
      id: "mock-ref-doc-2",
      collectionId: "mock-collection-2",
      name: "概要设计说明书",
      type: "document",
      locator: { kind: "path", path: "/Users/demo/docs/概要设计.md" },
    }),
    health: "unknown",
  },
  // data
  {
    ref: makeRef({
      id: "mock-ref-data-1",
      collectionId: "mock-collection-1",
      name: "样本数据集",
      type: "data",
      confidentiality: "sensitive",
      lifecycle: "staged",
      locator: { kind: "path", path: "/Users/demo/data/sample.parquet" },
    }),
    health: "unknown",
  },
];

function buildReferencesByType(collectionId: string): Record<ReferenceType, ReferenceWithHealth[]> {
  const grouped = {} as Record<ReferenceType, ReferenceWithHealth[]>;
  for (const t of ALL_REFERENCE_TYPES) grouped[t] = [];
  for (const item of seedReferences) {
    if (item.ref.collectionId === collectionId) {
      grouped[item.ref.type].push(item);
    }
  }
  return grouped;
}

/* ---------------- 引用 mock API ---------------- */

let mockRefSeq = 100;

function makeRefId(): string {
  return `mock-ref-${++mockRefSeq}`;
}

export const mockReferenceApi = {
  /**
   * 仅关联创建引用 — §2.5 ref_create_external。
   * mock 不真正访问文件系统，但模拟契约错误：
   * - 路径为空 / 非绝对路径 → COMMON_INVALID_PARAM
   * - 路径包含 "missing" → FS_PATH_NOT_FOUND（便于联调错误分支）
   */
  ref_create_external(input: RefCreateExternalInput): Reference {
    const collection = collectionStore.find((c) => c.id === input.collectionId);
    if (!collection) {
      throw { code: "COMMON_NOT_FOUND", message: "资源集不存在", retryable: false };
    }
    if (!input.name || input.name.trim() === "") {
      throw { code: "COMMON_INVALID_PARAM", message: "名称不能为空", retryable: false };
    }
    if (input.locator.kind !== "path") {
      throw {
        code: "COMMON_INVALID_PARAM",
        message: "仅支持 path 类型 locator",
        retryable: false,
      };
    }
    const path = input.locator.path;
    if (!path || !path.startsWith("/")) {
      throw {
        code: "COMMON_INVALID_PARAM",
        message: "路径必须为绝对路径",
        retryable: false,
      };
    }
    if (path.includes("missing")) {
      throw {
        code: "FS_PATH_NOT_FOUND",
        message: `路径不存在：${path}`,
        details: { path },
        retryable: false,
      };
    }

    const now = unixNow();
    const ref: Reference = {
      id: makeRefId(),
      collectionId: input.collectionId,
      sourceId: "mock-source-local",
      name: input.name.trim(),
      type: input.type,
      hosting: "external",
      locator: input.locator,
      description: input.description,
      tags: input.tags,
      lifecycle: input.lifecycle ?? "active",
      confidentiality: input.confidentiality ?? "internal",
      indexed: input.indexed ?? true,
      disposition: "none",
      createdAt: now,
      updatedAt: now,
    };
    seedReferences = [...seedReferences, { ref, health: "unknown" }];
    return ref;
  },
};
