import { TodoListPanel } from "../components/TodoListPanel";

/**
 * 全局待办页（M7-2 · 任务包 m7-7.2）。
 *
 * 展示全局 + 各空间 todo，支持按空间筛选。
 * 空间详情页"待办" tab 复用 TodoListPanel 并锁定 spaceId。
 */
export function TodoPage() {
  return (
    <div style={{ maxWidth: 960 }}>
      <h1 style={{ fontSize: 20, marginTop: 0, marginBottom: 16 }}>待办</h1>
      <TodoListPanel />
    </div>
  );
}
