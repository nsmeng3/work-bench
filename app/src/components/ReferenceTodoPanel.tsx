import { useCallback, useEffect, useState } from "react";
import {
  AutoComplete,
  Button,
  Empty,
  List,
  Space as AntSpace,
  Tag,
  Typography,
  message,
} from "antd";
import { CheckSquareOutlined, PlusOutlined } from "@ant-design/icons";
import type { Reference, Todo, TodoStatus } from "../api";
import { todoCreate, todoLinkRef, todoList, todoListByRef, todoUnlinkRef, toApiError } from "../api";

const { Text } = Typography;

const STATUS_LABEL: Record<TodoStatus, string> = {
  pending: "待办",
  doing: "进行中",
  done: "已完成",
  cancelled: "已取消",
};

const STATUS_COLOR: Record<TodoStatus, string | undefined> = {
  pending: undefined,
  doing: "processing",
  done: "success",
  cancelled: "default",
};

/**
 * 资源详情面板「关联待办」区块（M7-2 · 任务包 m7-7.2）。
 *
 * - 列出当前引用被哪些 todo 挂载（点击可解除）
 * - 提供「挂到待办」选择器：从现有 todo 中选，或输入标题快速新建
 *
 * 注意：仅接受 resource_reference.id（不接受 inbox_item.id），
 * 因为 todo_ref_link.ref_id 有外键约束。
 */
export interface ReferenceTodoPanelProps {
  reference: Reference;
  /** 数据变化后回调（通常用于触发外层刷新） */
  onChanged?: () => void;
}

export function ReferenceTodoPanel({ reference, onChanged }: ReferenceTodoPanelProps) {
  const [linked, setLinked] = useState<Todo[]>([]);
  const [loading, setLoading] = useState(true);
  /** 候选 todo（未挂载到当前 ref 的） */
  const [candidates, setCandidates] = useState<Todo[]>([]);
  /** 输入框值（可选现有 todo 标题，或新标题） */
  const [inputValue, setInputValue] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [messageApi, messageContextHolder] = message.useMessage();

  const fetchLinked = useCallback(async () => {
    setLoading(true);
    try {
      const list = await todoListByRef(reference.id);
      setLinked(list);
    } catch (err) {
      const apiErr = toApiError(err);
      messageApi.error({ content: apiErr.message, duration: 3 });
    } finally {
      setLoading(false);
    }
  }, [reference.id, messageApi]);

  const fetchCandidates = useCallback(async () => {
    try {
      // 拉所有活跃 todo（不含 done/cancelled），由用户挑选
      const all = await todoList({});
      setCandidates(all);
    } catch (err) {
      console.warn("加载候选 todo 失败：", err);
    }
  }, []);

  useEffect(() => {
    fetchLinked();
    fetchCandidates();
  }, [fetchLinked, fetchCandidates]);

  const linkedIds = new Set(linked.map((t) => t.id));
  const availableCandidates = candidates.filter((t) => !linkedIds.has(t.id));

  async function handleAttachExisting(todoId: string) {
    setSubmitting(true);
    try {
      await todoLinkRef({ todoId, refId: reference.id });
      messageApi.success({ content: "已挂载", duration: 1.5 });
      setInputValue("");
      await fetchLinked();
      await fetchCandidates();
      onChanged?.();
    } catch (err) {
      const apiErr = toApiError(err);
      messageApi.error({ content: apiErr.message, duration: 3 });
    } finally {
      setSubmitting(false);
    }
  }

  async function handleCreateAndAttach() {
    const title = inputValue.trim();
    if (!title) return;
    setSubmitting(true);
    try {
      const todo = await todoCreate({ title });
      await todoLinkRef({ todoId: todo.id, refId: reference.id });
      messageApi.success({ content: "已新建并挂载", duration: 1.5 });
      setInputValue("");
      await fetchLinked();
      await fetchCandidates();
      onChanged?.();
    } catch (err) {
      const apiErr = toApiError(err);
      messageApi.error({ content: apiErr.message, duration: 3 });
    } finally {
      setSubmitting(false);
    }
  }

  async function handleDetach(todoId: string) {
    setSubmitting(true);
    try {
      await todoUnlinkRef({ todoId, refId: reference.id });
      messageApi.success({ content: "已解除挂载", duration: 1.5 });
      await fetchLinked();
      await fetchCandidates();
      onChanged?.();
    } catch (err) {
      const apiErr = toApiError(err);
      messageApi.error({ content: apiErr.message, duration: 3 });
    } finally {
      setSubmitting(false);
    }
  }

  const exactMatch = availableCandidates.find((t) => t.title === inputValue.trim());

  return (
    <div>
      {messageContextHolder}
      <Text strong>关联待办（{linked.length}）</Text>
      {loading ? (
        <div style={{ padding: "12px 0" }}>
          <Text type="secondary" style={{ fontSize: 12 }}>
            加载中…
          </Text>
        </div>
      ) : linked.length === 0 ? (
        <Empty
          image={Empty.PRESENTED_IMAGE_SIMPLE}
          description="尚未被任何待办引用"
          style={{ padding: "12px 0" }}
        />
      ) : (
        <List
          size="small"
          style={{ marginTop: 8 }}
          dataSource={linked}
          renderItem={(todo) => (
            <List.Item
              actions={[
                <Button
                  key="detach"
                  type="text"
                  size="small"
                  danger
                  disabled={submitting}
                  onClick={() => void handleDetach(todo.id)}
                >
                  解除
                </Button>,
              ]}
            >
              <AntSpace size={8}>
                <CheckSquareOutlined />
                <span>{todo.title}</span>
                <Tag color={STATUS_COLOR[todo.status]} style={{ marginInlineEnd: 0 }}>
                  {STATUS_LABEL[todo.status]}
                </Tag>
              </AntSpace>
            </List.Item>
          )}
        />
      )}

      <AntSpace.Compact style={{ width: "100%", marginTop: 8 }}>
        <AutoComplete
          style={{ flex: 1 }}
          value={inputValue}
          onChange={setInputValue}
          placeholder="选择已有待办，或输入新标题"
          options={availableCandidates.map((t) => ({ value: t.title, label: t.title }))}
          filterOption={(input, option) =>
            (option?.value as string)?.toLowerCase().includes(input.toLowerCase()) ?? false
          }
          onSelect={(value) => {
            const hit = availableCandidates.find((t) => t.title === value);
            if (hit) void handleAttachExisting(hit.id);
          }}
        />
        {exactMatch ? (
          <Button
            type="primary"
            onClick={() => void handleAttachExisting(exactMatch.id)}
            loading={submitting}
          >
            挂载
          </Button>
        ) : (
          <Button
            type="primary"
            icon={<PlusOutlined />}
            onClick={() => void handleCreateAndAttach()}
            loading={submitting}
            disabled={!inputValue.trim()}
          >
            新建并挂载
          </Button>
        )}
      </AntSpace.Compact>
      <div style={{ marginTop: 4 }}>
        <Text type="secondary" style={{ fontSize: 12 }}>
          输入标题：选中已有待办直接挂载；无匹配时点击「新建并挂载」创建全局待办。
        </Text>
      </div>
    </div>
  );
}
