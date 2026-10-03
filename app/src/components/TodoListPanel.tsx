import { useState, useEffect, useCallback, useMemo } from "react";
import dayjs from "dayjs";
import type { Dayjs } from "dayjs";
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
  Collection,
  Reference,
  Space,
  Todo,
  TodoPatch,
  TodoPriority,
  TodoStatus,
  TodoWithRefs,
} from "../api";
import {
  collectionList,
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
import { ReferencePicker } from "./ReferencePicker";

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
 * - `spaceId` / `collectionId` 传入时锁定归属（空间/资源集详情页）；
 *   缺省时显示筛选下拉（全部 / 全局 / 各空间 / 各资源集）。
 * - 快速输入框：回车即创建，归属跟随锁定值或当前筛选。
 * - 行内标签：资源集（紫）> 空间（蓝）> 全局；空间随资源集自动派生。
 * - 每行：checkbox 切 done、标题、挂载资源数 icon、所属空间 tag。
 * - 已完成 / 已取消默认收起到底部分组。
 */
export interface TodoListPanelProps {
  /** 锁定空间：传入时仅显示该空间 todo，新建 todo 自动带上 spaceId */
  spaceId?: string;
  /** m8-8.5 · 锁定资源集：传入时仅显示该资源集 todo，新建自动带上 collectionId */
  collectionId?: string;
  /** 锁定资源集时用于行内标签展示 */
  collectionName?: string;
  /** 可选：外部传入可选空间列表（缺省时组件自行加载） */
  spaces?: Space[];
  /** 点击引用跳转（可选） */
  onOpenReference?: (ref: Reference) => void;
}

export function TodoListPanel({ spaceId, collectionId, collectionName, spaces: spacesProp, onOpenReference }: TodoListPanelProps) {
  const [todos, setTodos] = useState<Todo[]>([]);
  const [doneTodos, setDoneTodos] = useState<Todo[]>([]);
  const [loading, setLoading] = useState(true);
  const [spaces, setSpaces] = useState<Space[]>(spacesProp ?? []);
  /** m8-8.5 · 非锁定时加载全部资源集（筛选下拉 + 行内标签用） */
  const [collections, setCollections] = useState<Collection[]>([]);
  /** 仅当未锁定 spaceId 时生效：筛选下拉（"all" | "global" | spaceId） */
  const [spaceFilter, setSpaceFilter] = useState<string>("all");
  /** 仅当未锁定 collectionId 时生效：资源集筛选（"all" | collectionId） */
  const [collectionFilter, setCollectionFilter] = useState<string>("all");

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
    collectionId?: string | null;
    priority: TodoPriority;
    dueAt?: Dayjs | null;
    status: TodoStatus;
  }>();
  /** m8-8.5 · 详情抽屉的资源集选项（随所选空间联动加载） */
  const [collectionOptions, setCollectionOptions] = useState<Collection[]>([]);

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
        if (cancelled) return;
        setSpaces(list);
        // 加载资源集名（筛选下拉 + 行内标签；锁定资源集时用 collectionName 兜底）
        if (!collectionId) {
          const cols = await Promise.all(
            list.map((s) => collectionList({ spaceId: s.id, status: "active" })),
          );
          if (!cancelled) setCollections(cols.flat());
        }
      } catch (err) {
        console.warn("加载空间列表失败：", err);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [spacesProp, collectionId]);

  const spaceNameById = useMemo(() => {
    const m = new Map<string, string>();
    for (const s of spaces) m.set(s.id, s.name);
    return m;
  }, [spaces]);

  const collectionNameById = useMemo(() => {
    const m = new Map<string, string>();
    for (const c of collections) m.set(c.id, c.name);
    return m;
  }, [collections]);

  /** 当前生效的 spaceId 过滤（锁定优先；资源集筛选生效时空间由后端派生，无需传） */
  const effectiveSpaceId: string | undefined = useMemo(() => {
    if (spaceId) return spaceId;
    if (spaceFilter === "all") return undefined;
    if (spaceFilter === "global") return "global";
    return spaceFilter;
  }, [spaceId, spaceFilter]);

  /** 当前生效的 collectionId 过滤（锁定优先） */
  const effectiveCollectionId: string | undefined = useMemo(() => {
    if (collectionId) return collectionId;
    return collectionFilter !== "all" ? collectionFilter : undefined;
  }, [collectionId, collectionFilter]);

  const fetchTodos = useCallback(async () => {
    setLoading(true);
    try {
      const active = await todoList({ spaceId: effectiveSpaceId, collectionId: effectiveCollectionId });
      const finished = await todoList({ spaceId: effectiveSpaceId, collectionId: effectiveCollectionId, status: "all" });
      setTodos(active);
      setDoneTodos(finished.filter((t) => t.status === "done" || t.status === "cancelled"));
    } catch (err) {
      const apiErr = toApiError(err);
      messageApi.error({ content: apiErr.message, duration: 3 });
    } finally {
      setLoading(false);
    }
  }, [effectiveSpaceId, effectiveCollectionId, messageApi]);

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
        // 归属：锁定的空间/资源集 > 当前筛选（选了资源集时空间由后端派生）
        spaceId: effectiveCollectionId
          ? undefined
          : spaceId ??
            (spaceFilter !== "all" && spaceFilter !== "global" ? spaceFilter : undefined),
        collectionId: effectiveCollectionId,
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
        collectionId: d.collectionId ?? null,
        priority: d.priority,
        dueAt: d.dueAt ? dayjs.unix(d.dueAt) : null,
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
      collectionId: values.collectionId ?? null,
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

  /* ---------------- m8-8.5 · 详情抽屉资源集选择 ---------------- */

  /**
   * 抽屉打开时加载全部空间的资源集（按空间分组展示）。
   * 资源集本身就在空间下：选中资源集即自动带出所属空间，
   * 无需先选空间。
   */
  useEffect(() => {
    if (!detailId) return;
    let cancelled = false;
    (async () => {
      try {
        const lists = await Promise.all(
          spaces.map((s) => collectionList({ spaceId: s.id, status: "active" })),
        );
        if (!cancelled) setCollectionOptions(lists.flat());
      } catch (err) {
        console.warn("加载资源集列表失败：", err);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [detailId, spaces]);

  /** 资源集 → 所属空间 id 的映射（选中资源集时回填空间） */
  const collectionSpaceById = useMemo(() => {
    const m = new Map<string, string>();
    for (const c of collectionOptions) m.set(c.id, c.spaceId);
    return m;
  }, [collectionOptions]);

  /* ---------------- 挂载 / 卸载引用 ---------------- */

  async function handleAttachRef(ref: Reference) {
    if (!detailId) return;
    try {
      await todoLinkRef({ todoId: detailId, refId: ref.id });
      messageApi.success({ content: `已挂载「${ref.name}」`, duration: 1.5 });
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
          {todo.collectionId ? (
            <Tag color="purple" style={{ marginInlineEnd: 0 }}>
              {collectionNameById.get(todo.collectionId) ?? collectionName ?? "资源集"}
            </Tag>
          ) : spaceName ? (
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
          {!spaceId && !collectionId && (
            <>
              <Select
                value={spaceFilter}
                onChange={(v) => {
                  setSpaceFilter(v);
                  // 空间筛选变化时清掉资源集筛选，避免矛盾组合
                  setCollectionFilter("all");
                }}
                style={{ minWidth: 140 }}
                options={[
                  { value: "all", label: "全部空间" },
                  { value: "global", label: "仅全局" },
                  ...spaces.map((s) => ({ value: s.id, label: s.name })),
                ]}
              />
              <Select
                value={collectionFilter}
                onChange={setCollectionFilter}
                showSearch
                optionFilterProp="label"
                style={{ minWidth: 160 }}
                options={[
                  { value: "all", label: "全部资源集" },
                  ...spaces
                    .map((s) => ({
                      label: s.name,
                      options: collections
                        .filter((c) => c.spaceId === s.id)
                        .map((c) => ({ value: c.id, label: c.name })),
                    }))
                    .filter((g) => g.options.length > 0),
                ]}
              />
            </>
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
            collectionId
              ? `回车快速添加（归属资源集「${collectionName ?? ""}」）`
              : effectiveCollectionId
                ? `回车快速添加到「${collectionNameById.get(effectiveCollectionId) ?? ""}」`
                : spaceId
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
                  onChange={() => detailForm.setFieldValue("collectionId", null)}
                />
              </Form.Item>
              <Form.Item name="collectionId" label="所属资源集">
                <Select
                  allowClear
                  showSearch
                  optionFilterProp="label"
                  placeholder="不挂资源集"
                  options={spaces
                    .map((s) => ({
                      label: s.name,
                      options: collectionOptions
                        .filter((c) => c.spaceId === s.id)
                        .map((c) => ({ value: c.id, label: c.name })),
                    }))
                    .filter((g) => g.options.length > 0)}
                  onChange={(v) => {
                    // 选中资源集 → 空间自动跟随（资源集本身就在空间下）
                    if (v) {
                      const sid = collectionSpaceById.get(v);
                      if (sid) detailForm.setFieldValue("spaceId", sid);
                    }
                  }}
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
              <div style={{ marginTop: 8 }}>
                <ReferencePicker
                  attachedRefIds={detail.refs.map((r) => r.id)}
                  onSelect={(ref) => void handleAttachRef(ref)}
                />
              </div>
              <Text type="secondary" style={{ fontSize: 12, display: "block", marginTop: 4 }}>
                点击资源即挂载；也可在资源详情面板反向操作「挂到待办」。
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
