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
  ManagedPlan,
  Reference,
  ReferenceType,
  ReferenceWithHealth,
  RefCreateExternalInput,
  RefCreateManagedInput,
  RefUpdateInput,
  QueryRefsInput,
  QueryRefsOutput,
  QueryFacetsInput,
  QueryFacetsOutput,
  FacetValue,
  RootDirStatus,
  InitRootDirInput,
  InitRootDirResult,
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

/**
 * 筛选页 mock 引用池 — 覆盖六类型 / 多标签 / 多生命周期 / 多保密级别，
 * 用于 query_refs / query_facets 的 mock 数据源。
 * 与 collection_get 共用的 seedReferences 解耦，避免影响既有用例。
 */
const filterSeedReferences: Reference[] = (() => {
  const base = 1754039000;
  const list: Reference[] = [];
  const types: ReferenceType[] = ["code", "document", "data", "artifact", "tool", "media"];
  const lifecycles: Reference["lifecycle"][] = ["active", "staged", "delivered", "archived"];
  const confs: Reference["confidentiality"][] = [
    "public",
    "internal",
    "customer_restricted",
    "sensitive",
  ];
  const tagPool = ["frontend", "backend", "tauri", "design", "docs", "research", "ops"];
  const collections = ["mock-collection-1", "mock-collection-2"];

  for (let i = 0; i < 42; i++) {
    const type = types[i % types.length];
    const lifecycle = lifecycles[i % lifecycles.length];
    const confidentiality = confs[i % confs.length];
    const tags = [tagPool[i % tagPool.length]];
    if (i % 3 === 0) tags.push(tagPool[(i + 2) % tagPool.length]);
    if (i % 7 === 0) tags.push("hot");
    list.push(
      makeRef({
        id: `mock-filter-ref-${i + 1}`,
        collectionId: collections[i % collections.length],
        name: `${type}-示例-${i + 1}`,
        type,
        lifecycle,
        confidentiality,
        tags,
        description: `筛选 mock 第 ${i + 1} 条`,
        createdAt: base + i * 60,
        updatedAt: base + i * 60,
      }),
    );
  }
  return list;
})();

function matchKeyword(ref: Reference, keyword: string): boolean {
  const k = keyword.trim().toLowerCase();
  if (!k) return true;
  return (
    ref.name.toLowerCase().includes(k) ||
    (ref.description ?? "").toLowerCase().includes(k) ||
    (ref.tags ?? []).some((t) => t.toLowerCase().includes(k))
  );
}

export const mockQueryApi = {
  query_refs(input: QueryRefsInput): QueryRefsOutput {
    let items = filterSeedReferences.slice();
    if (input.collectionId) items = items.filter((r) => r.collectionId === input.collectionId);
    if (input.type) items = items.filter((r) => r.type === input.type);
    if (input.lifecycle) items = items.filter((r) => r.lifecycle === input.lifecycle);
    if (input.confidentiality)
      items = items.filter((r) => r.confidentiality === input.confidentiality);
    if (input.disposition) items = items.filter((r) => r.disposition === input.disposition);
    if (input.sourceId) items = items.filter((r) => r.sourceId === input.sourceId);
    if (input.tags && input.tags.length > 0) {
      // AND 语义：必须同时包含所有选中标签
      items = items.filter((r) => {
        const have = new Set(r.tags ?? []);
        return input.tags!.every((t) => have.has(t));
      });
    }
    if (input.keyword) items = items.filter((r) => matchKeyword(r, input.keyword!));

    const total = items.length;
    const offset = Math.max(0, input.offset ?? 0);
    const limit = Math.max(1, input.limit ?? 50);
    return { total, items: items.slice(offset, offset + limit) };
  },

  query_facets(_input: QueryFacetsInput): QueryFacetsOutput {
    // 注：mock 仅支持全量计数，不按 spaceId 过滤（mock 数据未挂 space 维度）。
    const count = <K extends keyof Reference>(
      key: K,
      pick: (v: Reference[K]) => string | undefined,
    ): FacetValue[] => {
      const m = new Map<string, number>();
      for (const r of filterSeedReferences) {
        const v = pick(r[key]);
        if (!v) continue;
        m.set(v, (m.get(v) ?? 0) + 1);
      }
      return Array.from(m.entries())
        .map(([value, count]) => ({ value, count }))
        .sort((a, b) => b.count - a.count);
    };
    const tagMap = new Map<string, number>();
    for (const r of filterSeedReferences) {
      for (const t of r.tags ?? []) tagMap.set(t, (tagMap.get(t) ?? 0) + 1);
    }
    return {
      types: count("type", (v) => v),
      lifecycles: count("lifecycle", (v) => v),
      confidentialities: count("confidentiality", (v) => v),
      tags: Array.from(tagMap.entries())
        .map(([value, count]) => ({ value, count }))
        .sort((a, b) => b.count - a.count),
    };
  },
};

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

/* ---------------- 设置中心 mock（§2.8） ---------------- */

/**
 * mock 状态：模块级变量，模拟 settings 表中的 root_dir 行。
 * 初始化为 null 表示「未初始化」，与首次启动场景一致。
 */
let mockRootDir: string | null = null;

const MOCK_TYPE_SUBDIRS = ["Code", "Documents", "Data", "Artifacts", "Tools", "Media"];

export const mockSettingsApi = {
  settings_get_root_dir(): RootDirStatus {
    if (mockRootDir === null) return { initialized: false };
    return { rootDir: mockRootDir, initialized: true };
  },

  settings_init_root_dir(input: InitRootDirInput): InitRootDirResult {
    const rootDir = input.rootDir.trim();
    if (!rootDir) {
      throw {
        code: "COMMON_INVALID_PARAM",
        message: "根目录不能为空",
        retryable: false,
      };
    }
    if (!rootDir.startsWith("/")) {
      throw {
        code: "COMMON_INVALID_PARAM",
        message: "根目录必须为绝对路径",
        retryable: false,
      };
    }
    if (rootDir.includes("deny")) {
      throw {
        code: "FS_PERMISSION_DENIED",
        message: `无写入权限：${rootDir}`,
        details: { path: rootDir },
        retryable: false,
      };
    }
    if (rootDir.includes("retry")) {
      throw {
        code: "FS_IO_ERROR",
        message: `IO 异常：${rootDir}`,
        details: { path: rootDir },
        retryable: true,
      };
    }
    mockRootDir = rootDir;
    const created = MOCK_TYPE_SUBDIRS.map((s) => `${rootDir.replace(/\/$/, "")}/${s}`);
    return { rootDir, created };
  },
};

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

  /**
   * §2.5 ref_update：仅允许修改管理属性；type/locator/hosting 不可改。
   * tags 为全量替换语义。
   */
  ref_update(input: RefUpdateInput): Reference {
    const idx = seedReferences.findIndex((r) => r.ref.id === input.id);
    if (idx === -1)
      throw { code: "COMMON_NOT_FOUND", message: "引用不存在", retryable: false };
    const item = seedReferences[idx];
    if (input.name !== undefined && input.name.trim() === "")
      throw { code: "COMMON_INVALID_PARAM", message: "名称不能为空", retryable: false };
    const updated: Reference = {
      ...item.ref,
      name: input.name ?? item.ref.name,
      description: input.description ?? item.ref.description,
      tags: input.tags ?? item.ref.tags,
      lifecycle: input.lifecycle ?? item.ref.lifecycle,
      confidentiality: input.confidentiality ?? item.ref.confidentiality,
      indexed: input.indexed ?? item.ref.indexed,
      updatedAt: unixNow(),
    };
    seedReferences = [
      ...seedReferences.slice(0, idx),
      { ...item, ref: updated },
      ...seedReferences.slice(idx + 1),
    ];
    return updated;
  },

  /**
   * §2.5 ref_create_managed confirmed=false — 返回 ManagedPlan，不做写操作。
   * mock 形状与契约逐字段一致：
   * - source 来自入参 locator.path
   * - proposedTarget 拼成 `<rootDir|/mock-root>/<Type 子目录>/<源名>`
   * - sizeBytes / fileCount 用确定性伪随机（按路径 hash），便于联调
   * - 路径含 "missing" → FS_PATH_NOT_FOUND
   * - 路径含 "conflict" → 返回非空 conflicts，便于联调冲突分支
   */
  ref_create_managed_plan(input: RefCreateManagedInput): ManagedPlan {
    const collection = collectionStore.find((c) => c.id === input.collectionId);
    if (!collection) {
      throw { code: "COMMON_NOT_FOUND", message: "资源集不存在", retryable: false };
    }
    if (input.locator.kind !== "path") {
      throw {
        code: "COMMON_INVALID_PARAM",
        message: "仅支持 path 类型 locator",
        retryable: false,
      };
    }
    const source = input.locator.path;
    if (!source || !source.startsWith("/")) {
      throw {
        code: "COMMON_INVALID_PARAM",
        message: "路径必须为绝对路径",
        retryable: false,
      };
    }
    if (source.includes("missing")) {
      throw {
        code: "FS_PATH_NOT_FOUND",
        message: `路径不存在：${source}`,
        details: { path: source },
        retryable: false,
      };
    }
    // 类型 → 子目录名（与 settings_init_root_dir 的 MOCK_TYPE_SUBDIRS 对齐）
    const TYPE_DIR: Record<ReferenceType, string> = {
      code: "Code",
      document: "Documents",
      data: "Data",
      artifact: "Artifacts",
      tool: "Tools",
      media: "Media",
    };
    const baseName = source.split("/").filter(Boolean).pop() ?? "unnamed";
    const root = mockRootDir ?? "/mock-root";
    const proposedTarget = `${root}/${TYPE_DIR[input.type]}/${baseName}`;

    // 确定性伪随机 size/fileCount（按路径字符串 hash）
    let hash = 0;
    for (let i = 0; i < source.length; i++) hash = (hash * 31 + source.charCodeAt(i)) >>> 0;
    const isDir = !/\.[a-z0-9]{1,5}$/i.test(baseName);
    const fileCount = isDir ? (hash % 200) + 3 : 1;
    const sizeBytes = isDir ? 1024 * 1024 * ((hash % 500) + 10) : 1024 * ((hash % 2048) + 1);

    const conflicts: string[] = [];
    if (source.includes("conflict")) {
      conflicts.push(`目标已存在同名项：${proposedTarget}`);
    }

    return {
      kind: "managed_plan",
      source,
      proposedTarget,
      action: input.managedAction,
      sizeBytes,
      fileCount,
      conflicts,
    };
  },

  /**
   * §2.5 ref_create_managed confirmed=true — 执行 copy/move 并落库。
   * mock 不做真实文件操作，仅生成 hosting=managed 的 Reference，
   * locator.path 指向最终目标路径（含 targetName 覆盖）。
   * 路径含 "io-error" → COMMON_IO，便于联调错误分支。
   */
  ref_create_managed_confirmed(input: RefCreateManagedInput): Reference {
    const collection = collectionStore.find((c) => c.id === input.collectionId);
    if (!collection) {
      throw { code: "COMMON_NOT_FOUND", message: "资源集不存在", retryable: false };
    }
    if (input.locator.kind !== "path") {
      throw {
        code: "COMMON_INVALID_PARAM",
        message: "仅支持 path 类型 locator",
        retryable: false,
      };
    }
    const source = input.locator.path;
    if (source.includes("missing")) {
      throw {
        code: "FS_PATH_NOT_FOUND",
        message: `路径不存在：${source}`,
        details: { path: source },
        retryable: false,
      };
    }
    if (source.includes("io-error")) {
      throw {
        code: "COMMON_IO",
        message: `IO 异常：${source}`,
        details: { path: source },
        retryable: true,
      };
    }
    if (input.targetName && input.targetName.includes("exists")) {
      throw {
        code: "FS_TARGET_EXISTS",
        message: `目标已存在：${input.targetName}`,
        details: { targetName: input.targetName },
        retryable: false,
      };
    }

    const TYPE_DIR: Record<ReferenceType, string> = {
      code: "Code",
      document: "Documents",
      data: "Data",
      artifact: "Artifacts",
      tool: "Tools",
      media: "Media",
    };
    const baseName = source.split("/").filter(Boolean).pop() ?? "unnamed";
    const finalName = input.targetName?.trim() || baseName;
    const root = mockRootDir ?? "/mock-root";
    const finalTarget = `${root}/${TYPE_DIR[input.type]}/${finalName}`;

    const now = unixNow();
    const ref: Reference = {
      id: makeRefId(),
      collectionId: input.collectionId,
      sourceId: "mock-source-local",
      name: input.name.trim(),
      type: input.type,
      hosting: "managed",
      locator: { kind: "path", path: finalTarget },
      description: input.description,
      tags: input.tags,
      lifecycle: input.lifecycle ?? "active",
      confidentiality: input.confidentiality ?? "internal",
      indexed: input.indexed ?? true,
      disposition: "none",
      createdAt: now,
      updatedAt: now,
    };
    seedReferences = [...seedReferences, { ref, health: "ok" }];
    return ref;
  },
};
