import { invoke, Channel } from "@tauri-apps/api/core";
import type {
  TerminalEvent,
  TerminalCreateInput,
  TerminalCreateOutput,
  TerminalListItem,
} from "./types";

/**
 * 内嵌终端 API — 任务包 m7-7.4。
 *
 * 注意：终端**不支持** VITE_MOCK_API 切换（mock 无法跑真 PTY）。
 * 在 mock 模式下调用会抛错；调用方需自行隐藏终端 tab。
 */

function assertNotMock() {
  if (import.meta.env.VITE_MOCK_API === "true") {
    throw new Error("终端在 mock 模式下不可用");
  }
}

/**
 * 创建终端会话。`onEvent` 是 Tauri Channel，后端会持续推送 Data / Exited。
 */
export async function terminalCreate(
  input: TerminalCreateInput,
  onEvent: Channel<TerminalEvent>,
): Promise<TerminalCreateOutput> {
  assertNotMock();
  return invoke<TerminalCreateOutput>("terminal_create", {
    spaceId: input.spaceId,
    cwd: input.cwd,
    cols: input.cols,
    rows: input.rows,
    onEvent,
  });
}

export async function terminalWrite(sessionId: string, data: string): Promise<void> {
  assertNotMock();
  return invoke<void>("terminal_write", { sessionId, data });
}

export async function terminalResize(
  sessionId: string,
  cols: number,
  rows: number,
): Promise<void> {
  assertNotMock();
  return invoke<void>("terminal_resize", { sessionId, cols, rows });
}

export async function terminalClose(sessionId: string): Promise<void> {
  assertNotMock();
  return invoke<void>("terminal_close", { sessionId });
}

export async function terminalList(): Promise<TerminalListItem[]> {
  assertNotMock();
  return invoke<TerminalListItem[]>("terminal_list");
}
