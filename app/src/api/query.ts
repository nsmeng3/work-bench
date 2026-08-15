import { invoke } from "@tauri-apps/api/core";
import type {
  QueryRefsInput,
  QueryRefsOutput,
  QueryFacetsInput,
  QueryFacetsOutput,
} from "./types";
import { mockQueryApi } from "./mock";

const MOCK = import.meta.env.VITE_MOCK_API === "true";

/**
 * 查询筛选 API — 契约见详细设计说明书 §2.9。
 * 后端未就绪时可通过 VITE_MOCK_API=true 切换到 mock 数据。
 */

export async function queryRefs(input: QueryRefsInput): Promise<QueryRefsOutput> {
  if (MOCK) return mockQueryApi.query_refs(input);
  return invoke<QueryRefsOutput>("query_refs", { ...input });
}

export async function queryFacets(input: QueryFacetsInput): Promise<QueryFacetsOutput> {
  if (MOCK) return mockQueryApi.query_facets(input);
  return invoke<QueryFacetsOutput>("query_facets", { ...input });
}
