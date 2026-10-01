import { invoke } from "@tauri-apps/api/core";
import type {
  Todo,
  TodoWithRefs,
  TodoCreateInput,
  TodoPatch,
  TodoListInput,
  TodoSetStatusInput,
  TodoRefLinkInput,
  TodoStatus,
} from "./types";
import { mockTodoApi } from "./mock";

const MOCK = import.meta.env.VITE_MOCK_API === "true";

/**
 * 待办 API — 任务包 m7-7.2。
 * 后端未就绪时可通过 VITE_MOCK_API=true 切换到 mock 数据。
 */

export async function todoList(input: TodoListInput = {}): Promise<Todo[]> {
  if (MOCK) return mockTodoApi.todo_list(input);
  return invoke<Todo[]>("todo_list", { ...input });
}

export async function todoGet(id: string): Promise<TodoWithRefs> {
  if (MOCK) return mockTodoApi.todo_get(id);
  return invoke<TodoWithRefs>("todo_get", { id });
}

export async function todoCreate(input: TodoCreateInput): Promise<Todo> {
  if (MOCK) return mockTodoApi.todo_create(input);
  return invoke<Todo>("todo_create", { input });
}

export async function todoUpdate(id: string, patch: TodoPatch): Promise<Todo> {
  if (MOCK) return mockTodoApi.todo_update(id, patch);
  return invoke<Todo>("todo_update", { id, patch });
}

export async function todoSetStatus(input: TodoSetStatusInput): Promise<Todo> {
  if (MOCK) return mockTodoApi.todo_set_status(input);
  return invoke<Todo>("todo_set_status", { ...input });
}

export async function todoDelete(id: string): Promise<void> {
  if (MOCK) return mockTodoApi.todo_delete(id);
  return invoke<void>("todo_delete", { id });
}

export async function todoLinkRef(input: TodoRefLinkInput): Promise<void> {
  if (MOCK) return mockTodoApi.todo_link_ref(input);
  return invoke<void>("todo_link_ref", { ...input });
}

export async function todoUnlinkRef(input: TodoRefLinkInput): Promise<void> {
  if (MOCK) return mockTodoApi.todo_unlink_ref(input);
  return invoke<void>("todo_unlink_ref", { ...input });
}

export async function todoListByRef(refId: string): Promise<Todo[]> {
  if (MOCK) return mockTodoApi.todo_list_by_ref(refId);
  return invoke<Todo[]>("todo_list_by_ref", { refId });
}

/** 便捷封装：切换 done / pending */
export async function todoToggleDone(todo: Todo): Promise<Todo> {
  const next: TodoStatus = todo.status === "done" ? "pending" : "done";
  return todoSetStatus({ id: todo.id, status: next });
}
