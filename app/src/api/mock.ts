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
  DispositionAudit,
  DispAuditListInput,
  DispCapabilities,
  DispPreview,
  DispDestroyResult,
  UndoPlan,
  InboxItem,
  InboxItemDetail,
  InboxListInput,
  InboxSnoozeInput,
  InboxIgnoreInput,
  InboxStats,
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
  // m4-4.7 三档按钮 mock：archived / deleted / 大目录
  {
    ref: makeRef({
      id: "mock-ref-archived-1",
      collectionId: "mock-collection-1",
      name: "已归档的旧文档",
      type: "document",
      disposition: "archived",
      locator: { kind: "path", path: "/Users/demo/docs/old-archived.md" },
    }),
    health: "ok",
  },
  {
    ref: makeRef({
      id: "mock-ref-deleted-1",
      collectionId: "mock-collection-1",
      name: "已删除到回收站",
      type: "media",
      disposition: "deleted",
      locator: { kind: "path", path: "/Users/demo/media/deleted.mov" },
    }),
    health: "missing",
  },
  {
    ref: makeRef({
      id: "mock-ref-bigdir-1",
      collectionId: "mock-collection-1",
      name: "大型代码仓",
      type: "code",
      description: "用于演示大目录 preview 加载",
      locator: { kind: "path", path: "/Users/demo/code/big-repo" },
    }),
    health: "ok",
  },
  // m4-4.9 撤销导入 mock：4 种场景
  {
    ref: makeRef({
      id: "mock-ref-undo-copy-ok",
      collectionId: "mock-collection-1",
      name: "可撤销的copy导入.pdf",
      type: "document",
      hosting: "managed",
      locator: {
        kind: "path",
        path: "/mock-root/Documents/可撤销的copy导入.pdf",
        originalSource: "/Users/demo/Downloads/可撤销的copy导入.pdf",
        managedAction: "copy",
      },
      createdAt: Math.floor(Date.now() / 1000) - 3600, // 1 小时前
      updatedAt: Math.floor(Date.now() / 1000) - 3600,
    }),
    health: "ok",
  },
  {
    ref: makeRef({
      id: "mock-ref-undo-expired",
      collectionId: "mock-collection-1",
      name: "超过24h的导入.pdf",
      type: "document",
      hosting: "managed",
      locator: {
        kind: "path",
        path: "/mock-root/Documents/超过24h的导入.pdf",
        originalSource: "/Users/demo/Downloads/超过24h的导入.pdf",
        managedAction: "copy",
      },
      createdAt: Math.floor(Date.now() / 1000) - 25 * 3600, // 25 小时前
      updatedAt: Math.floor(Date.now() / 1000) - 25 * 3600,
    }),
    health: "ok",
  },
  {
    ref: makeRef({
      id: "mock-ref-undo-no-source",
      collectionId: "mock-collection-1",
      name: "M3老数据无originalSource.pdf",
      type: "document",
      hosting: "managed",
      locator: {
        kind: "path",
        path: "/mock-root/Documents/M3老数据无originalSource.pdf",
        // 无 originalSource / managedAction（M3 期间创建）
      },
      createdAt: Math.floor(Date.now() / 1000) - 3600,
      updatedAt: Math.floor(Date.now() / 1000) - 3600,
    }),
    health: "ok",
  },
  {
    ref: makeRef({
      id: "mock-ref-undo-move-occupied",
      collectionId: "mock-collection-1",
      name: "move撤销源被占.pdf",
      type: "document",
      hosting: "managed",
      locator: {
        kind: "path",
        path: "/mock-root/Documents/move撤销源被占.pdf",
        originalSource: "/mock-occupied/move撤销源被占.pdf",
        managedAction: "move",
      },
      createdAt: Math.floor(Date.now() / 1000) - 3600,
      updatedAt: Math.floor(Date.now() / 1000) - 3600,
    }),
    health: "ok",
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

/* ---------------- 处置审计 mock（§2.6 disp_audit_list） ---------------- */

/**
 * 审计 mock 数据：覆盖 4 种 action，含已销毁（refId 不在 seedReferences 中）与未销毁条目。
 * `at` 使用固定基准 + 递偏移，保证按时间倒序时次序稳定。
 */
const seedDispositionAudits: DispositionAudit[] = (() => {
  const base = 1754039000;
  const list: DispositionAudit[] = [];
  const actions: Array<DispositionAudit["action"]> = [
    "archive",
    "unarchive",
    "soft_delete",
    "destroy",
  ];
  // 已销毁的引用（模拟 refId 已不在 resource_reference 中，但审计仍存活 §6.7）
  const destroyedRefs = [
    { id: "mock-destroyed-ref-1", name: "旧报告.pdf" },
    { id: "mock-destroyed-ref-2", name: "临时数据集" },
  ];
  // 未销毁的引用（来自 filterSeedReferences 前几条，便于联调「点击跳详情」）
  const liveRefs = filterSeedReferences.slice(0, 4).map((r) => ({ id: r.id, name: r.name }));

  let seq = 0;
  // 每种 action 各造 3 条；前 2 条用 liveRefs，最后 1 条用 destroyedRefs
  for (const action of actions) {
    for (let i = 0; i < 3; i++) {
      const ref = i < 2 ? liveRefs[(seq + i) % liveRefs.length] : destroyedRefs[seq % destroyedRefs.length];
      list.push({
        id: `mock-audit-${action}-${i + 1}`,
        refId: ref.id,
        refName: ref.name,
        action,
        locatorSnapshot: { kind: "path", path: `/mock/path/${ref.name}` },
        actor: "local_user",
        note:
          action === "destroy"
            ? JSON.stringify({ isDir: false, fileCount: 1, totalBytes: 1024 * (seq + 1) })
            : null,
        at: base + seq * 60,
      });
      seq += 1;
    }
  }
  return list;
})();


/* ---------------- 处置 mock（§2.6 · m4-4.7） ---------------- */

/**
 * 计算引用的处置能力 — 与后端 `compute_capabilities` 纯函数规则一致。
 * mock 默认 softDelete 支持（macOS）；路径含 "no-recycle" 时模拟不支持。
 */
function computeMockCapabilities(ref: Reference): DispCapabilities {
  const reason: DispCapabilities["reason"] = {};
  const disposition = ref.disposition;
  const softDeleteSupported = !(
    ref.locator.kind === "path" && ref.locator.path.includes("no-recycle")
  );

  const archive = disposition !== "archived";
  if (!archive) reason.archive = "已归档，无需重复归档";

  const softDelete = softDeleteSupported;
  if (!softDelete) reason.softDelete = "当前存储源不支持系统回收站";

  const destroy = true;

  const restoreFromBin = disposition === "deleted";
  if (!restoreFromBin) reason.restoreFromBin = "引用未处于回收站状态";

  return { archive, softDelete, destroy, restoreFromBin, reason };
}

function findMockRef(refId: string): Reference {
  const item = seedReferences.find((r) => r.ref.id === refId);
  if (!item) throw { code: "COMMON_NOT_FOUND", message: "资源引用不存在", retryable: false };
  return item.ref;
}

function updateMockRefDisposition(refId: string, disposition: Reference["disposition"]): Reference {
  const idx = seedReferences.findIndex((r) => r.ref.id === refId);
  if (idx === -1)
    throw { code: "COMMON_NOT_FOUND", message: "资源引用不存在", retryable: false };
  const item = seedReferences[idx];
  const updated: Reference = { ...item.ref, disposition, updatedAt: unixNow() };
  seedReferences = [
    ...seedReferences.slice(0, idx),
    { ...item, ref: updated },
    ...seedReferences.slice(idx + 1),
  ];
  return updated;
}

function humanSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let v = bytes;
  let u = -1;
  do {
    v /= 1024;
    u += 1;
  } while (v >= 1024 && u < units.length - 1);
  return `${v.toFixed(2)} ${units[u]}`;
}

export const mockDispositionApi = {
  disp_get_capabilities(refId: string): DispCapabilities {
    const ref = findMockRef(refId);
    return computeMockCapabilities(ref);
  },

  disp_archive(refId: string): Reference {
    const ref = findMockRef(refId);
    if (ref.disposition !== "none") {
      throw {
        code: "COMMON_CONFLICT",
        message: `当前 disposition=${ref.disposition}，无法执行 archive（要求 none）`,
        retryable: false,
      };
    }
    return updateMockRefDisposition(refId, "archived");
  },

  disp_unarchive(refId: string): Reference {
    const ref = findMockRef(refId);
    if (ref.disposition !== "archived") {
      throw {
        code: "COMMON_CONFLICT",
        message: `当前 disposition=${ref.disposition}，无法执行 unarchive（要求 archived）`,
        retryable: false,
      };
    }
    return updateMockRefDisposition(refId, "none");
  },

  disp_soft_delete(refId: string): Reference {
    const ref = findMockRef(refId);
    const caps = computeMockCapabilities(ref);
    if (!caps.softDelete) {
      throw {
        code: "FS_RECYCLE_UNSUPPORTED",
        message: "当前系统不支持回收站，只能归档或销毁",
        retryable: false,
      };
    }
    return updateMockRefDisposition(refId, "deleted");
  },

  /**
   * mock preview：按 refId 路径 hash 生成确定性伪随机统计；
   * 路径含 "big-repo" 模拟大目录（fileCount 较大）。
   */
  disp_preview(refId: string): DispPreview {
    const ref = findMockRef(refId);
    const target = ref.locator.kind === "path" ? ref.locator.path : `/mock/${ref.id}`;
    let hash = 0;
    for (let i = 0; i < target.length; i++) hash = (hash * 31 + target.charCodeAt(i)) >>> 0;
    const isDir = !/\.[a-z0-9]{1,5}$/i.test(target);
    const isBig = target.includes("big-repo");
    const fileCount = isBig ? 1234 : isDir ? (hash % 200) + 3 : 1;
    const totalBytes = isBig
      ? 1024 * 1024 * 5678
      : isDir
        ? 1024 * 1024 * ((hash % 500) + 10)
        : 1024 * ((hash % 2048) + 1);
    const warning = isDir
      ? `目录包含 ${fileCount} 个文件，共 ${humanSize(totalBytes)}；销毁后不可恢复`
      : `文件大小 ${humanSize(totalBytes)}；销毁后不可恢复`;
    return {
      previewId: `mock-preview-${refId}`,
      target,
      isDir,
      fileCount,
      totalBytes,
      capability: computeMockCapabilities(ref),
      warning,
    };
  },

  disp_destroy(refId: string, confirmText: string): DispDestroyResult {
    const ref = findMockRef(refId);
    if (confirmText !== ref.name) {
      throw {
        code: "COMMON_CONFIRM_REQUIRED",
        message: `confirmText 与资源名称不一致（期望: ${ref.name}）`,
        retryable: false,
      };
    }
    // 物理删除引用行
    seedReferences = seedReferences.filter((r) => r.ref.id !== refId);
    return { deletedRefId: refId };
  },

  /**
   * §2.6 disp_audit_list mock：按 refId / action 过滤，按 at DESC 排序，支持分页。
   * action 非法值 → COMMON_INVALID_PARAM（与后端行为对齐）。
   */
  disp_audit_list(input: DispAuditListInput): DispositionAudit[] {
    const VALID: DispositionAudit["action"][] = [
      "archive",
      "unarchive",
      "soft_delete",
      "destroy",
      "undo_import",
    ];
    if (input.action && !VALID.includes(input.action)) {
      throw {
        code: "COMMON_INVALID_PARAM",
        message: `非法 action: ${input.action}`,
        retryable: false,
      };
    }
    let items = seedDispositionAudits.slice();
    if (input.refId) items = items.filter((a) => a.refId === input.refId);
    if (input.action) items = items.filter((a) => a.action === input.action);
    items.sort((a, b) => b.at - a.at);
    const offset = Math.max(0, input.offset ?? 0);
    const limit = Math.min(200, Math.max(1, input.limit ?? 50));
    return items.slice(offset, offset + limit);
  },

  /**
   * m4-4.9 · ref_undo_import mock。
   *
   * 覆盖 4 种场景（按 refId 匹配）：
   * - `mock-ref-undo-copy-ok`：可撤销（copy，24h 内，有 originalSource）
   * - `mock-ref-undo-expired`：超 24h 窗口 → canUndo=false
   * - `mock-ref-undo-no-source`：无 originalSource（M3 老数据）→ canUndo=false
   * - `mock-ref-undo-move-occupied`：move 且 originalSource 被占 → canUndo=false
   *
   * 其他 managed 引用：按通用规则计算 blockers。
   * confirmed=true 时模拟执行：从 seedReferences 中移除引用，返回 null。
   */
  ref_undo_import(refId: string, confirmed: boolean): UndoPlan | null {
    const ref = findMockRef(refId);
    const now = Math.floor(Date.now() / 1000);
    const UNDO_WINDOW_SECS = 24 * 3600;

    const blockers: string[] = [];
    const locator = ref.locator;
    const currentPath = locator.kind === "path" ? locator.path : `/mock/${ref.id}`;
    const originalSource =
      locator.kind === "path" ? (locator.originalSource ?? null) : null;
    const managedAction =
      locator.kind === "path" ? (locator.managedAction ?? "copy") : "copy";

    // 1. hosting 检查
    if (ref.hosting !== "managed") {
      blockers.push("仅 managed 导入的引用可撤销");
    }
    // 2. originalSource 缺失
    if (!originalSource) {
      blockers.push("该引用创建时未记录原始源，无法撤销");
    }
    // 3. 24h 窗口
    if (now - ref.createdAt > UNDO_WINDOW_SECS) {
      blockers.push(`引用创建已超过 24 小时撤销窗口（创建于 ${ref.createdAt}）`);
    }
    // 4. move 场景：originalSource 被占（mock 规则：路径含 "mock-occupied"）
    if (managedAction === "move" && originalSource?.includes("mock-occupied")) {
      blockers.push(`原始源路径已被其他文件占用: ${originalSource}`);
    }

    const plan: UndoPlan = {
      refId: ref.id,
      refName: ref.name,
      managedAction,
      currentPath,
      originalSource,
      canUndo: blockers.length === 0,
      blockers,
    };

    if (!confirmed) return plan;

    // confirmed=true
    if (!plan.canUndo) {
      throw {
        code: "COMMON_FORBIDDEN",
        message: `撤销被阻止: ${blockers.join("；")}`,
        retryable: false,
      };
    }
    // 模拟执行：从 seedReferences 中移除引用
    seedReferences = seedReferences.filter((r) => r.ref.id !== refId);
    return null;
  },
};

/* ---------------- 收件箱 mock（§2.7 · m5-5.6） ---------------- */

/**
 * 收件箱 mock 数据：覆盖 5 种 status × 多种 suggestedType × 敏感/非敏感。
 * discoveredAt 使用相对当前时间的偏移，保证「3 分钟前」等相对时间显示自然。
 */
const seedInboxItems: InboxItem[] = (() => {
  const now = Math.floor(Date.now() / 1000);
  const list: InboxItem[] = [
    {
      id: "mock-inbox-1",
      watchDirId: "mock-watch-1",
      path: "/Users/demo/Downloads/季度报告-2026Q2.pdf",
      eventKind: "created",
      sizeBytes: 1024 * 1024 * 3 + 512 * 1024, // 3.5 MB
      mtime: now - 180,
      ext: "pdf",
      suggestedType: "document",
      status: "pending",
      discoveredAt: now - 180, // 3 分钟前
    },
    {
      id: "mock-inbox-2",
      watchDirId: "mock-watch-1",
      path: "/Users/demo/Downloads/frontend-snapshot.tar.gz",
      eventKind: "created",
      sizeBytes: 1024 * 1024 * 48,
      mtime: now - 900,
      ext: "gz",
      suggestedType: "code",
      status: "pending",
      discoveredAt: now - 900, // 15 分钟前
    },
    {
      id: "mock-inbox-3",
      watchDirId: "mock-watch-1",
      path: "/Users/demo/Downloads/身份证扫描件.jpg",
      eventKind: "created",
      sizeBytes: 1024 * 512,
      mtime: now - 3600,
      ext: "jpg",
      suggestedType: "media",
      status: "pending",
      discoveredAt: now - 3600, // 1 小时前
    },
    {
      id: "mock-inbox-4",
      watchDirId: "mock-watch-1",
      path: "/Users/demo/Downloads/meeting-notes.md",
      eventKind: "modified",
      sizeBytes: 1024 * 12,
      mtime: now - 7200,
      ext: "md",
      suggestedType: "document",
      status: "snoozed",
      snoozeNote: "等下周例会前再处理",
      remindAt: now + 86400 * 3,
      discoveredAt: now - 7200, // 2 小时前
    },
    {
      id: "mock-inbox-5",
      watchDirId: "mock-watch-1",
      path: "/Users/demo/Downloads/temp-dataset.csv",
      eventKind: "created",
      sizeBytes: 1024 * 256,
      mtime: now - 86400,
      ext: "csv",
      suggestedType: "data",
      status: "snoozed",
      snoozeNote: "需要先确认数据来源",
      discoveredAt: now - 86400, // 1 天前
    },
    {
      id: "mock-inbox-6",
      watchDirId: "mock-watch-1",
      path: "/Users/demo/Downloads/.DS_Store",
      eventKind: "created",
      sizeBytes: 6148,
      mtime: now - 86400 * 2,
      ext: undefined,
      suggestedType: undefined,
      status: "ignored",
      discoveredAt: now - 86400 * 2, // 2 天前
    },
    {
      id: "mock-inbox-7",
      watchDirId: "mock-watch-1",
      path: "/Users/demo/Downloads/老照片.zip",
      eventKind: "created",
      sizeBytes: 1024 * 1024 * 220,
      mtime: now - 86400 * 3,
      ext: "zip",
      suggestedType: "media",
      status: "ignored",
      ignoreRuleId: "mock-ignore-rule-1",
      discoveredAt: now - 86400 * 3, // 3 天前
    },
    {
      id: "mock-inbox-8",
      watchDirId: "mock-watch-1",
      path: "/Users/demo/Downloads/合同草稿-v3.docx",
      eventKind: "renamed",
      sizeBytes: 1024 * 88,
      mtime: now - 86400 * 4,
      ext: "docx",
      suggestedType: "document",
      status: "processed",
      assignJson: JSON.stringify({
        mode: "external",
        spaceId: "mock-space-1",
        collectionId: "mock-collection-2",
        type: "document",
        referenceId: "mock-ref-doc-1",
        processedAt: now - 86400 * 4 + 60,
      }),
      discoveredAt: now - 86400 * 4, // 4 天前
    },
  ];
  return list;
})();

let inboxStore: InboxItem[] = [...seedInboxItems];

/** 敏感文件 mock 规则：文件名含「身份证 / 合同 / 护照 / 银行」返回提示 */
function mockSensitiveWarning(item: InboxItem): string | null {
  const sensitive = ["身份证", "护照", "银行卡", "合同"];
  const name = item.path.split("/").pop() ?? item.path;
  if (item.status !== "pending" && item.status !== "snoozed") return null;
  if (sensitive.some((k) => name.includes(k))) {
    return `该文件可能包含敏感信息（${name}），预览已禁用`;
  }
  return null;
}

/** 文本预览 mock：仅对文本类扩展名（md/txt/csv/json/log）返回前 N 行 */
function mockPreview(item: InboxItem): InboxItemDetail["preview"] {
  if (mockSensitiveWarning(item)) return null;
  const textExts = ["md", "txt", "csv", "json", "log"];
  if (!item.ext || !textExts.includes(item.ext)) return null;
  const name = item.path.split("/").pop() ?? item.path;
  const lines = [
    `# ${name} 预览（mock）`,
    ``,
    `这是 mock 数据，用于联调收件箱详情面板。`,
    `实际预览由后端 5.5 敏感识别扩展点提供。`,
    `路径：${item.path}`,
    `大小：${item.sizeBytes ?? 0} 字节`,
    `建议类型：${item.suggestedType ?? "未识别"}`,
  ];
  return { kind: "text", lines, truncated: false };
}

function toInboxDetail(item: InboxItem): InboxItemDetail {
  return {
    ...item,
    preview: mockPreview(item),
    sensitiveWarning: mockSensitiveWarning(item),
  };
}

export const mockInboxApi = {
  inbox_list(input: InboxListInput): InboxItem[] {
    let items = inboxStore.slice();
    if (input.status) items = items.filter((i) => i.status === input.status);
    // 与后端 ORDER BY discovered_at DESC 对齐
    items.sort((a, b) => b.discoveredAt - a.discoveredAt);
    const offset = Math.max(0, input.offset ?? 0);
    const limit = Math.min(200, Math.max(1, input.limit ?? 50));
    return items.slice(offset, offset + limit);
  },

  inbox_get(id: string): InboxItemDetail {
    const item = inboxStore.find((i) => i.id === id);
    if (!item) throw { code: "COMMON_NOT_FOUND", message: "收件箱条目不存在", retryable: false };
    return toInboxDetail(item);
  },

  inbox_snooze(input: InboxSnoozeInput): InboxItem {
    const idx = inboxStore.findIndex((i) => i.id === input.id);
    if (idx === -1)
      throw { code: "COMMON_NOT_FOUND", message: "收件箱条目不存在", retryable: false };
    const updated: InboxItem = {
      ...inboxStore[idx],
      status: "snoozed",
      snoozeNote: input.note ?? null,
      remindAt: input.remindAt ?? null,
    };
    inboxStore = [
      ...inboxStore.slice(0, idx),
      updated,
      ...inboxStore.slice(idx + 1),
    ];
    return updated;
  },

  inbox_ignore(input: InboxIgnoreInput): InboxItem {
    const idx = inboxStore.findIndex((i) => i.id === input.id);
    if (idx === -1)
      throw { code: "COMMON_NOT_FOUND", message: "收件箱条目不存在", retryable: false };
    const kind = input.rule?.kind ?? "once";
    const updated: InboxItem = {
      ...inboxStore[idx],
      status: "ignored",
      ignoreRuleId: kind === "once" ? null : `mock-ignore-rule-${input.id}`,
    };
    inboxStore = [
      ...inboxStore.slice(0, idx),
      updated,
      ...inboxStore.slice(idx + 1),
    ];
    return updated;
  },

  inbox_stats(): InboxStats {
    const pending = inboxStore.filter((i) => i.status === "pending").length;
    const snoozed = inboxStore.filter((i) => i.status === "snoozed").length;
    const lastEventAt =
      inboxStore.length > 0
        ? Math.max(...inboxStore.map((i) => i.discoveredAt))
        : null;
    return { pending, snoozed, lastEventAt };
  },
};