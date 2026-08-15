import type { Space, SpaceCreateInput, SpaceUpdateInput, SpaceIdInput, SpaceListInput } from "./types";

/**
 * Mock 数据 — 形状与契约逐字段一致（§2.3 / §3.2 space 表）。
 * 仅在 VITE_MOCK_API 开启时使用，默认关闭。
 */

let mockSeq = 100;

function isoNow(): string {
  return new Date().toISOString();
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
    createdAt: "2026-08-01T09:00:00Z",
    updatedAt: "2026-08-01T09:00:00Z",
  },
  {
    id: "mock-space-2",
    name: "生活",
    description: "个人生活资源",
    color: "#7BC47F",
    icon: "home",
    status: "active",
    createdAt: "2026-08-01T09:01:00Z",
    updatedAt: "2026-08-01T09:01:00Z",
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
    const now = isoNow();
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
      updatedAt: isoNow(),
    };
    store[idx] = updated;
    return updated;
  },

  space_archive(input: SpaceIdInput): Space {
    const idx = store.findIndex((s) => s.id === input.id);
    if (idx === -1) throw { code: "COMMON_NOT_FOUND", message: "空间不存在", retryable: false };
    const s = store[idx];
    if (s.status === "archived") throw { code: "COMMON_CONFLICT", message: "空间已归档", retryable: false };
    const updated: Space = { ...s, status: "archived", updatedAt: isoNow() };
    store[idx] = updated;
    return updated;
  },

  space_restore(input: SpaceIdInput): Space {
    const idx = store.findIndex((s) => s.id === input.id);
    if (idx === -1) throw { code: "COMMON_NOT_FOUND", message: "空间不存在", retryable: false };
    const updated: Space = { ...store[idx], status: "active", updatedAt: isoNow() };
    store[idx] = updated;
    return updated;
  },
};
