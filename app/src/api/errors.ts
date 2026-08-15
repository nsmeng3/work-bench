import type { ApiError } from "./types";

/**
 * 将 Tauri invoke 抛出的错误统一解析为 ApiError。
 * 后端按 §2.2 约定返回 { code, message, details, retryable }，
 * 此处做兜底：非结构化错误包装为 COMMON_UNKNOWN。
 */
export function toApiError(err: unknown): ApiError {
  if (err !== null && typeof err === "object" && "code" in err && "message" in err) {
    const e = err as Record<string, unknown>;
    return {
      code: String(e.code),
      message: String(e.message),
      details: (e.details as Record<string, unknown>) ?? undefined,
      retryable: Boolean(e.retryable),
    };
  }
  return {
    code: "COMMON_UNKNOWN",
    message: err instanceof Error ? err.message : String(err),
    retryable: false,
  };
}
