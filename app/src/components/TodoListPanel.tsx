import { useState, useEffect, useCallback, useMemo } from "react";
import {
  Button,
  Checkbox,
  DatePicker,
  Drawer,
  Empty,
  Form,
  Input,
  List,
  Popconfirm,
  Select,
  Space as AntSpace,
  Spin,
  Tag,
  Typography,
  message,
} from "antd";
import {
  DeleteOutlined,
  PaperClipOutlined,
  PlusOutlined,
  ReloadOutlined,
} from "@ant-design/icons";
import type {
  Reference,
  Space,
  Todo,
  TodoPatch,
  TodoPriority,
  TodoStatus,
  TodoWithRefs,
} from "../api";
import {
  spaceList,
  todoCreate,
  todoDelete,
  todoGet,
  todoLinkRef,
  todoList,
  todoSetStatus,
  todoUnlinkRef,
  todoUpdate,
  toApiError,
} from "../api";

const { Text } = Typography;

/** 状态 → 中文标签 */
const STATUS_LABEL: Record<TodoStatus, string> = {
  pending: "待办",
  doing: "进行中",
  done: "已完成",
  cancelled: "已取消",
};

/** 优先级 → 中文 + 颜色 */
const PRIORITY_META: Record<TodoPriority, { label: string; color?: string }> = {
  0: { label: "普通" },
  1: { label: "重要", color: "orange" },
  2: { label: "紧急", color: "red" },
};

function formatUnixSeconds(ts?: number): string {
  if (ts === undefined || !Number.isFinite(ts)) return "-";
  return new Date(ts * 1000).toLocaleString();
}

function formatShortDate(ts?: number): string {
  if (ts === undefined || !Number.isFinite(ts)) return "";
  const d = new Date(ts * 1000);
  const month = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${month}-${day}`;
}

/**
 * 待办列表面板（受控组件）。
 *
 * - `spaceId` 传入时锁定该空间（用于空间详情页"待办" tab）；
 *   缺省时显示筛选下拉（全部 / 全局 / 各空间）。
 * - 快速输入框：回车即创建（默认全局，除非 `spaceId` 锁定）。
 * - 每行：checkbox 切 done、标题、挂载资源数 icon、所属空间 tag。
 * - 已完成 / 已取消默认收起到底部分组。
 */
export interface TodoListPanelProps {
  /** 锁定空间：传入时仅显示该空间 todo，新建 todo 自动带上 spaceId */
  spaceId?: string;
  /** 可选：外部传入可选空间列表（缺省时组件自行加载） */
  spaces?: Space[];
  /** 点击引用跳转（可选） */
  onOpenReference?: (ref: Reference) => void;
}

export function TodoListPanel({ spaceId, spaces: spacesProp, onOpenReference }: TodoListPanelProps) {
  const [todos, setTodos] = useState<Todo[]>([]);
  const [doneTodos, setDoneTodos] = useState<Todo[]>([]);
  const [loading, setLoading] = useState(true);
  const [spaces, setSpaces] = useState<Space[]>(spacesProp ?? []);
  /** 仅当未锁定 spaceId 时生效：筛选下拉（"all" | "global" | spaceId） */
  const [spaceFilter, setSpaceFilter] = useState<string>("all");

  const [quickTitle, setQuickTitle] = useState("");
  const [creating, setCreating] = useState(false);

  /** 详情 drawer */
  const [detailId, setDetailId] = useState<string | null>(null);
  const [detail, setDetail] = useState<TodoWithRefs | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [detailForm] = Form.useForm<{
    title: string;
    note?: string;
    spaceId?: string | null;
    priority: TodoPriority;
    dueAt?: { unix(): number } | null;
    status: TodoStatus;
  }>();

  /** 挂载资源 */
  const [attachRefId, setAttachRefId] = useState<string>("");

  const [messageApi, messageContextHolder] = message.useMessage();

  // 加载空间列表（用于筛选下拉 + tag 显示）
  useEffect(() => {
    if (spacesProp) {
      setSpaces(spacesProp);
      return;
    }
    let cancelled = false;
    (async () => {
      try {
        const list = await spaceList({ status: "active" });
        if (!cancelled) setSpaces(list);
      } catch (err) {
        console.warn("加载空间列表失败：", err);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [spacesProp]);

  const spaceNameById = useMemo(() => {
    const m = new Map<string, string>();
    for (const s of spaces) m.set(s.id, s.name);
    return m;
  }, [spaces]);

  /** 当前生效的 spaceId 过滤（锁定优先） */
  const effectiveSpaceId: string | undefined = useMemo(() => {
    if (spaceId) return spaceId;
    if (spaceFilter === "all") return undefined;
    if (spaceFilter === "global") return "global";
    return spaceFilter;
  }, [spaceId, spaceFilter]);

  const fetchTodos = useCallback(async () => {
    setLoading(true);
    try {
      const active = await todoList({ spaceId: effectiveSpaceId });
      const finished = await todoList({ spaceId: effectiveSpaceId, status: "all" });
      setTodos(active);
      setDoneTodos(finished.filter((t) => t.status === "done" || t.status === "cancelled"));
    } catch (err) {
      const apiErr = toApiError(err);
      messageApi.error({ content: apiErr.message, duration: 3 });
    } finally {
      setLoading(false);
    }
  }, [effectiveSpaceId, messageApi]);

  useEffect(() => {
    fetchTodos();
  }, [fetchTodos]);

  /* ---------------- 快速创建 ---------------- */

  async function handleQuickCreate() {
    const title = quickTitle.trim();
    if (!title) return;
    setCreating(true);
    try {
      await todoCreate({
        title,
        // 锁定时使用锁定空间；否则使用筛选值（all → 全局；global → 全局；其他 → 该空间）
        spaceId:
          spaceId ??
          (spaceFilter !== "all" && spaceFilter !== "global" ? spaceFilter : undefined),
      });
      setQuickTitle("");
      messageApi.success({ content: "已添加", duration: 1.5 });
      await fetchTodos();
    } catch (err) {
      const apiErr = toApiError(err);
      messageApi.error({ content: apiErr.message, duration: 3 });
    } finally {
      setCreating(false);
    }
  }

  /* ---------------- 状态切换 ---------------- */

  async function handleToggleDone(todo: Todo) {
    try {
      const next: TodoStatus = todo.status === "done" ? "pending" : "done";
      await todoSetStatus({ id: todo.id, status: next });
      await fetchTodos();
    } catch (err) {
      const apiErr = toApiError(err);
      messageApi.error({ content: apiErr.message, duration: 3 });
    }
  }

  /* ---------------- 详情 drawer ---------------- */

  async function reloadDetail(id: string) {
    setDetailLoading(true);
    try {
      const d = await todoGet(id);
      setDetail(d);
      detailForm.setFieldsValue({
        title: d.title,
        note: d.note,
        spaceId: d.spaceId ?? null,
        priority: d.priority,
        dueAt: d.dueAt ? ({ unix: () => d.dueAt! } as { unix(): number }) : null,
        status: d.status,
      });
    } catch (err) {
      const apiErr = toApiError(err);
      messageApi.error({ content: apiErr.message, duration: 3 });
    } finally {
      setDetailLoading(false);
    }
  }

  async function openDetail(todo: Todo) {
    setDetailId(todo.id);
    setDetail(null);
    await reloadDetail(todo.id);
  }

  function closeDetail() {
    setDetailId(null);
    setDetail(null);
    detailForm.resetFields();
    setAttachRefId("");
  }

  async function handleDetailSave() {
    if (!detailId) return;
    let values;
    try {
      values = await detailForm.validateFields();
    } catch {
      return;
    }
    setSaving(true);
    try {
      const patch: TodoPatch = {
      title: values.title.trim(),
      note: values.note?.trim() ? values.note.trim() : null,
      spaceId: values.spaceId ?? null,
      priority: values.priority,
      dueAt: values.dueAt ? values.dueAt.unix() : null,
      };
      await todoUpdate(detailId, patch);
      // 状态变更走专门命令
      if (detail && values.status !== detail.status) {
        await todoSetStatus({ id: detailId, status: values.status });
      }
      messageApi.success({ content: "已保存", duration: 2 });
      await fetchTodos();
      await reloadDetail(detailId);
    } catch (err) {
      const apiErr = toApiError(err);
      messageApi.error({ content: apiErr.message, duration: 3 });
    } finally {
      setSaving(false);
    }
  }

  async function handleDelete(todo: Todo) {
    try {
      await todoDelete(todo.id);
      messageApi.success({ content: "已删除", duration: 2 });
      if (detailId === todo.id) closeDetail();
      await fetchTodos();
    } catch (err) {
      const apiErr = toApiError(err);
      messageApi.error({ content: apiErr.message, duration: 3 });
    }
  }

  /* ---------------- 挂载 / 卸载引用 ---------------- */

  async function handleAttachRef() {
    if (!detailId || !attachRefId.trim()) return;
    try {
      await todoLinkRef({ todoId: detailId, refId: attachRefId.trim() });
      messageApi.success({ content: "已挂载", duration: 1.5 });
      setAttachRefId("");
      await reloadDetail(detailId);
      await fetchTodos();
    } catch (err) {
      const apiErr = toApiError(err);
      messageApi.error({ content: apiErr.message, duration: 3 });
    }
  }

  async function handleDetachRef(refId: string) {
    if (!detailId) return;
    try {
      await todoUnlinkRef({ todoId: detailId, refId });
      messageApi.success({ content: "已解除挂载", duration: 1.5 });
      await reloadDetail(detailId);
      await fetchTodos();
    } catch (err) {
      const apiErr = toApiError(err);
      messageApi.error({ content: apiErr.message, duration: 3 });
    }
  }

  /* ---------------- 渲染行 ---------------- */

  function renderRow(todo: Todo) {
    const isDone = todo.status === "done";
    const isCancelled = todo.status === "cancelled";
    const spaceName = todo.spaceId ? spaceNameById.get(todo.spaceId) : undefined;
    const priorityMeta = PRIORITY_META[todo.priority];

    return (
      <List.Item
        key={todo.id}
        onClick={() => void openDetail(todo)}
        style={{ cursor: "pointer", padding: "8px 12px" }}
        actions={[
          <Popconfirm
            key="del"
            title="删除该待办？"
            okText="删除"
            okButtonProps={{ danger: true }}
            cancelText="取消"
            onConfirm={(e) => {
              e?.stopPropagation();
              void handleDelete(todo);
            }}
            onCancel={(e) => e?.stopPropagation()}
          >
            <Button
              type="text"
              size="small"
              danger
              icon={<DeleteOutlined />}
              onClick={(e) => e.stopPropagation()}
            />
          </Popconfirm>,
        ]}
      >
        <AntSpace size={8} wrap style={{ flex: 1 }}>
          <Checkbox
            checked={isDone}
            onClick={(e) => e.stopPropagation()}
            onChange={() => void handleToggleDone(todo)}
          />
          <span
            style={{
              textDecoration: isDone || isCancelled ? "line-through" : undefined,
              color: isCancelled ? "#999" : undefined,
              fontWeight: 500,
            }}
          >
            {todo.title}
          </span>
          {todo.priority > 0 && (
            <Tag color={priorityMeta.color} style={{ marginInlineEnd: 0 }}>
              {priorityMeta.label}
            </Tag>
          )}
          {todo.status === "doing" && <Tag color="processing">进行中</Tag>}
          {isCancelled && <Tag>已取消</Tag>}
          {todo.dueAt && (
            <Text type="secondary" style={{ fontSize: 12 }}>
              截止 {formatShortDate(todo.dueAt)}
            </Text>
          )}
          <TodoRefCount todoId={todo.id} />
          {spaceName ? (
            <Tag color="blue" style={{ marginInlineEnd: 0 }}>
              {spaceName}
            </Tag>
          ) : (
            <Tag style={{ marginInlineEnd: 0 }}>全局</Tag>
          )}
        </AntSpace>
      </List.Item>
    );
  }

  return (
    <div>
      {messageContextHolder}

      {/* 顶部工具条 */}
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          marginBottom: 12,
          gap: 12,
          flexWrap: "wrap",
        }}
      >
        <AntSpace>
          {!spaceId && (
            <Select
              value={spaceFilter}
              onChange={setSpaceFilter}
              style={{ minWidth: 160 }}
              options={[
                { value: "all", label: "全部空间" },
                { value: "global", label: "仅全局" },
                ...spaces.map((s) => ({ value: s.id, label: s.name })),
              ]}
            />
          )}
          <Button icon={<ReloadOutlined />} onClick={fetchTodos} loading={loading}>
            刷新
          </Button>
        </AntSpace>
      </div>

      {/* 快速输入 */}
      <AntSpace.Compact style={{ width: "100%", marginBottom: 12 }}>
        <Input
          placeholder={
            spaceId
              ? "回车快速添加（归属当前空间）"
              : spaceFilter !== "all" && spaceFilter !== "global"
                ? `回车快速添加到「${spaceNameById.get(spaceFilter) ?? spaceFilter}」`
                : "回车快速添加（默认全局）"
          }
          value={quickTitle}
          onChange={(e) => setQuickTitle(e.target.value)}
          onPressEnter={() => void handleQuickCreate()}
          maxLength={200}
          disabled={creating}
        />
        <Button
          type="primary"
          icon={<PlusOutlined />}
          onClick={() => void handleQuickCreate()}
          loading={creating}
          disabled={!quickTitle.trim()}
        >
          添加
        </Button>
      </AntSpace.Compact>

      {loading && todos.length === 0 ? (
        <div style={{ textAlign: "center", padding: "32px 0" }}>
          <Spin />
        </div>
      ) : (
        <>
          <List
            dataSource={todos}
            renderItem={renderRow}
            locale={{ emptyText: <Empty description="暂无待办，从上方输入框快速添加" /> }}
            size="small"
            bordered
          />

          {doneTodos.length > 0 && (
            <div style={{ marginTop: 16 }}>
              <Text type="secondary" style={{ fontSize: 12 }}>
                已完成 / 已取消（{doneTodos.length}）
              </Text>
              <List
                dataSource={doneTodos}
                renderItem={renderRow}
                size="small"
                bordered
                style={{ marginTop: 8, opacity: 0.75 }}
              />
            </div>
          )}
        </>
      )}

      {/* 详情 drawer */}
      <Drawer
        title="待办详情"
        open={detailId !== null}
        onClose={closeDetail}
        width={520}
        destroyOnHidden
        footer={
          <AntSpace style={{ width: "100%", justifyContent: "flex-end" }}>
            <Button onClick={closeDetail}>关闭</Button>
            <Button type="primary" onClick={() => void handleDetailSave()} loading={saving}>
              保存
            </Button>
          </AntSpace>
        }
      >
        {detailLoading || !detail ? (
          <div style={{ textAlign: "center", padding: "32px 0" }}>
            <Spin />
          </div>
        ) : (
          <AntSpace direction="vertical" size={16} style={{ width: "100%" }}>
            <Form form={detailForm} layout="vertical">
              <Form.Item
                name="title"
                label="标题"
                rules={[{ required: true, message: "标题不能为空" }]}
              >
                <Input maxLength={200} />
              </Form.Item>
              <Form.Item name="note" label="备注">
                <Input.TextArea rows={3} maxLength={2000} />
              </Form.Item>
              <Form.Item name="status" label="状态" rules={[{ required: true }]}>
                <Select
                  options={(Object.keys(STATUS_LABEL) as TodoStatus[]).map((s) => ({
                    value: s,
                    label: STATUS_LABEL[s],
                  }))}
                />
              </Form.Item>
              <Form.Item name="priority" label="优先级" rules={[{ required: true }]}>
                <Select
                  options={([0, 1, 2] as TodoPriority[]).map((p) => ({
                    value: p,
                    label: PRIORITY_META[p].label,
                  }))}
                />
              </Form.Item>
              <Form.Item name="spaceId" label="所属空间">
                <Select
                  allowClear
                  placeholder="全局（不挂空间）"
                  options={spaces.map((s) => ({ value: s.id, label: s.name }))}
                />
              </Form.Item>
              <Form.Item name="dueAt" label="截止时间">
                <DatePicker showTime style={{ width: "100%" }} />
              </Form.Item>
            </Form>

            <div>
              <Text strong>挂载的资源（{detail.refs.length}）</Text>
              {detail.refs.length === 0 ? (
                <div style={{ marginTop: 8 }}>
                  <Text type="secondary" style={{ fontSize: 12 }}>
                    暂未挂载资源
                  </Text>
                </div>
              ) : (
                <List
                  size="small"
                  style={{ marginTop: 8 }}
                  dataSource={detail.refs}
                  renderItem={(ref) => (
                    <List.Item
                      actions={[
                        <Button
                          key="detach"
                          type="text"
                          size="small"
                          danger
                          onClick={() => void handleDetachRef(ref.id)}
                        >
                          解除
                        </Button>,
                      ]}
                    >
                      <AntSpace size={8}>
                        <PaperClipOutlined />
                        {onOpenReference ? (
                          <a onClick={() => onOpenReference(ref)}>{ref.name}</a>
                        ) : (
                          <span>{ref.name}</span>
                        )}
                        <Text type="secondary" style={{ fontSize: 12 }}>
                          {ref.type}
                        </Text>
                      </AntSpace>
                    </List.Item>
                  )}
                />
              )}
              <AntSpace.Compact style={{ width: "100%", marginTop: 8 }}>
                <Input
                  placeholder="输入引用 ID 挂载（高级）"
                  value={attachRefId}
                  onChange={(e) => setAttachRefId(e.target.value)}
                  onPressEnter={() => void handleAttachRef()}
                />
                <Button onClick={() => void handleAttachRef()} disabled={!attachRefId.trim()}>
                  挂载
                </Button>
              </AntSpace.Compact>
              <Text type="secondary" style={{ fontSize: 12 }}>
                提示：在资源详情面板使用「挂到待办」更直观。
              </Text>
            </div>

            <div>
              <Text type="secondary" style={{ fontSize: 12 }}>
                创建于 {formatUnixSeconds(detail.createdAt)} · 更新于{" "}
                {formatUnixSeconds(detail.updatedAt)}
                {detail.doneAt && ` · 完成于 ${formatUnixSeconds(detail.doneAt)}`}
              </Text>
            </div>
          </AntSpace>
        )}
      </Drawer>
    </div>
  );
}

/** 行内"挂载资源数"图标（异步拉一次详情拿 refs.length） */
function TodoRefCount({ todoId }: { todoId: string }) {
  const [count, setCount] = useState<number | null>(null);
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const d = await todoGet(todoId);
        if (!cancelled) setCount(d.refs.length);
      } catch {
        if (!cancelled) setCount(null);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [todoId]);
  if (!count) return null;
  return (
    <Text type="secondary" style={{ fontSize: 12 }}>
      <PaperClipOutlined /> {count}
    </Text>
  );
}
