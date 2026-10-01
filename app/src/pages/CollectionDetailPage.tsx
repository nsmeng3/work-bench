import { useState, useEffect, useCallback } from "react";
import {
  Badge,
  Button,
  Collapse,
  Descriptions,
  Dropdown,
  Empty,
  Form,
  Input,
  List,
  Modal,
  Select,
  Space as AntSpace,
  Spin,
  Switch,
  Tag,
  Typography,
  message,
} from "antd";
import {
  ArrowLeftOutlined,
  CodeOutlined,
  CopyOutlined,
  DownOutlined,
  EditOutlined,
  ExportOutlined,
  FolderOpenOutlined,
  PlayCircleOutlined,
  PlusOutlined,
  ReloadOutlined,
  RollbackOutlined,
} from "@ant-design/icons";
import type {
  Collection,
  CollectionDetail,
  Reference,
  ReferenceConfidentiality,
  ReferenceHealth,
  ReferenceLifecycle,
  ReferenceType,
  ReferenceWithHealth,
  Space,
} from "../api";
import {
  collectionGet,
  refCopyPath,
  refLogAccessSafe,
  refOpen,
  refOpenWith,
  refRevealInFinder,
  refOpenInTerminal,
  refUpdate,
  toApiError,
} from "../api";
import { ReferenceCreateDialog } from "../components/ReferenceCreateDialog";
import { ReferenceManagedDialog } from "../components/ReferenceManagedDialog";
import { ReferenceTodoPanel } from "../components/ReferenceTodoPanel";
import { DispositionButtons } from "../components/DispositionButtons";
import type { DispositionAction } from "../components/DispositionButtons";
import { DispositionConfirmDialog } from "../components/DispositionConfirmDialog";
import { UndoImportDialog } from "../components/UndoImportDialog";

const { Text, Paragraph } = Typography;

/** 六类型分组固定键序 — 详细设计说明书 §2.4 */
const TYPE_ORDER: ReferenceType[] = ["code", "document", "data", "artifact", "tool", "media"];

const TYPE_LABEL: Record<ReferenceType, string> = {
  code: "代码",
  document: "文档",
  data: "数据",
  artifact: "构建产物",
  tool: "工具",
  media: "媒体",
};

const HEALTH_META: Record<ReferenceHealth, { color: string; text: string }> = {
  ok: { color: "success", text: "正常" },
  missing: { color: "error", text: "失效" },
  unknown: { color: "default", text: "未检测" },
};

const LIFECYCLE_LABEL: Record<ReferenceLifecycle, string> = {
  active: "活跃",
  staged: "已暂存",
  delivered: "已交付",
  archived: "已归档",
};

const CONFIDENTIALITY_LABEL: Record<ReferenceConfidentiality, string> = {
  public: "公开",
  internal: "内部",
  customer_restricted: "客户受限",
  sensitive: "敏感",
};

const LIFECYCLE_OPTIONS: { value: ReferenceLifecycle; label: string }[] = (
  Object.keys(LIFECYCLE_LABEL) as ReferenceLifecycle[]
).map((v) => ({ value: v, label: LIFECYCLE_LABEL[v] }));

const CONFIDENTIALITY_OPTIONS: { value: ReferenceConfidentiality; label: string }[] = (
  Object.keys(CONFIDENTIALITY_LABEL) as ReferenceConfidentiality[]
).map((v) => ({ value: v, label: CONFIDENTIALITY_LABEL[v] }));

function formatUnixSeconds(ts: number): string {
  if (!Number.isFinite(ts)) return "-";
  return new Date(ts * 1000).toLocaleString();
}

function locatorText(ref: ReferenceWithHealth["ref"]): string {
  const loc = ref.locator;
  if (loc.kind === "path") return loc.path;
  if (loc.kind === "repo") return loc.local;
  return `${loc.provider}:${loc.objectId}`;
}

/** m4-4.9：判断引用是否可显示「撤销导入」按钮（hosting=managed + 24h 窗口内） */
function canShowUndoButton(ref: Reference): boolean {
  if (ref.hosting !== "managed") return false;
  const UNDO_WINDOW_SECS = 24 * 3600;
  const now = Math.floor(Date.now() / 1000);
  return now - ref.createdAt <= UNDO_WINDOW_SECS;
}

interface CollectionDetailPageProps {
  space: Space;
  collection: Collection;
  onBack: () => void;
}

/** 编辑表单字段 — §2.5 ref_update 仅允许管理属性 */
interface RefEditFormValues {
  name: string;
  description?: string;
  tags?: string[];
  lifecycle: ReferenceLifecycle;
  confidentiality: ReferenceConfidentiality;
  indexed: boolean;
}

export function CollectionDetailPage({ space, collection, onBack }: CollectionDetailPageProps) {
  const [detail, setDetail] = useState<CollectionDetail | null>(null);
  const [loading, setLoading] = useState(true);
  const [createOpen, setCreateOpen] = useState(false);
  const [managedOpen, setManagedOpen] = useState(false);
  const [editingRef, setEditingRef] = useState<Reference | null>(null);
  const [saving, setSaving] = useState(false);
  const [dispositionAction, setDispositionAction] = useState<DispositionAction | null>(null);
  const [dispositionRef, setDispositionRef] = useState<Reference | null>(null);
  const [undoRef, setUndoRef] = useState<Reference | null>(null);
  const [form] = Form.useForm<RefEditFormValues>();
  const [messageApi, messageContextHolder] = message.useMessage();

  const fetchDetail = useCallback(async () => {
    setLoading(true);
    try {
      const result = await collectionGet({ id: collection.id });
      setDetail(result);
    } catch (err) {
      const apiErr = toApiError(err);
      messageApi.error({
        content: apiErr.retryable ? `${apiErr.message}（可重试）` : apiErr.message,
        duration: 3,
      });
    } finally {
      setLoading(false);
    }
  }, [collection.id, messageApi]);

  useEffect(() => {
    fetchDetail();
  }, [fetchDetail]);

  const openEditModal = (ref: Reference) => {
    setEditingRef(ref);
    form.setFieldsValue({
      name: ref.name,
      description: ref.description,
      tags: ref.tags ?? [],
      lifecycle: ref.lifecycle,
      confidentiality: ref.confidentiality,
      indexed: ref.indexed,
    });
  };

  const closeEditModal = () => {
    if (saving) return;
    setEditingRef(null);
    form.resetFields();
  };

  const openDispositionDialog = (ref: Reference, action: DispositionAction) => {
    setDispositionRef(ref);
    setDispositionAction(action);
  };

  const closeDispositionDialog = () => {
    setDispositionAction(null);
    setDispositionRef(null);
  };

  const openUndoDialog = (ref: Reference) => {
    setUndoRef(ref);
  };

  const closeUndoDialog = () => {
    setUndoRef(null);
  };

  /* ---------------- M7-1 · 资源打开/操作 ---------------- */

  /**
   * 打开引用（默认策略：appOverride > 类型默认 > 系统默认）。
   * 成功后埋点 action="open"。
   */
  const handleOpen = async (ref: Reference) => {
    try {
      await refOpen(ref.id);
      void refLogAccessSafe(ref.id, "open");
    } catch (err) {
      const apiErr = toApiError(err);
      messageApi.error({
        content: `打开失败：${apiErr.message}`,
        duration: 3,
      });
    }
  };

  /** 在文件管理器中显示。成功后埋点 action="reveal"。 */
  const handleReveal = async (ref: Reference) => {
    try {
      await refRevealInFinder(ref.id);
      void refLogAccessSafe(ref.id, "reveal");
    } catch (err) {
      const apiErr = toApiError(err);
      messageApi.error({
        content: `定位失败：${apiErr.message}`,
        duration: 3,
      });
    }
  };

  /**
   * 唤起系统终端并 cd 到资源目录（m7-7.5）。
   * - 文件 → cd 到父目录；目录 → cd 到它本身
   * - 成功后埋点 action="open"（复用现有枚举，本质是"打开"的一种形态）
   */
  const handleOpenInTerminal = async (ref: Reference) => {
    try {
      await refOpenInTerminal(ref.id);
      void refLogAccessSafe(ref.id, "open");
    } catch (err) {
      const apiErr = toApiError(err);
      messageApi.error({
        content: `在终端打开失败：${apiErr.message}`,
        duration: 3,
      });
    }
  };

  /** 复制路径到剪贴板。成功后埋点 action="copy_path"。 */
  const handleCopyPath = async (ref: Reference) => {
    try {
      await refCopyPath(ref);
      messageApi.success({ content: "路径已复制", duration: 2 });
      void refLogAccessSafe(ref.id, "copy_path");
    } catch (err) {
      const apiErr = toApiError(err);
      messageApi.error({
        content: `复制失败：${apiErr.message}`,
        duration: 3,
      });
    }
  };

  /**
   * 用其他程序打开：调 Tauri dialog 让用户选 .app，再调 refOpenWith。
   * 成功后埋点 action="open_with"。
   * 在非 Tauri 环境（如纯浏览器 mock）dialog 会抛错，降级提示用户。
   */
  const handleOpenWith = async (ref: Reference) => {
    try {
      const dialog = await import("@tauri-apps/plugin-dialog");
      const selected = await dialog.open({
        directory: false,
        multiple: false,
        title: "选择用于打开的应用",
      });
      if (typeof selected !== "string" || !selected) return; // 用户取消
      await refOpenWith(ref.id, selected);
      void refLogAccessSafe(ref.id, "open_with");
    } catch (err) {
      const apiErr = toApiError(err);
      messageApi.error({
        content: `打开失败：${apiErr.message}`,
        duration: 3,
      });
    }
  };

  const handleSave = async () => {
    if (!editingRef) return;
    let values: RefEditFormValues;
    try {
      values = await form.validateFields();
    } catch {
      return; // 校验失败，antd 已提示
    }
    setSaving(true);
    try {
      await refUpdate({
        id: editingRef.id,
        name: values.name.trim(),
        description: values.description?.trim() || undefined,
        tags: values.tags ?? [],
        lifecycle: values.lifecycle,
        confidentiality: values.confidentiality,
        indexed: values.indexed,
      });
      messageApi.success({ content: "引用已更新", duration: 2 });
      setEditingRef(null);
      form.resetFields();
      await fetchDetail();
    } catch (err) {
      const apiErr = toApiError(err);
      messageApi.error({
        content: apiErr.retryable ? `${apiErr.message}（可重试）` : apiErr.message,
        duration: 3,
      });
    } finally {
      setSaving(false);
    }
  };

  const collapseItems = TYPE_ORDER.map((type) => {
    const items = detail?.referencesByType[type] ?? [];
    const count = items.length;
    return {
      key: type,
      label: (
        <AntSpace>
          <span>{TYPE_LABEL[type]}</span>
          <Badge count={count} showZero color={count > 0 ? "#4a90d9" : "#bfbfbf"} />
          <Text type="secondary" style={{ fontSize: 12 }}>
            {type}
          </Text>
        </AntSpace>
      ),
      children:
        count === 0 ? (
          <Empty
            image={Empty.PRESENTED_IMAGE_SIMPLE}
            description={`暂无${TYPE_LABEL[type]}类引用`}
            style={{ padding: "16px 0" }}
          />
        ) : (
          <List<ReferenceWithHealth>
            dataSource={items}
            renderItem={({ ref, health }) => {
              const meta = HEALTH_META[health];
              const isPathLocator = ref.locator.kind === "path";
              const contextMenuItems = [
                { key: "open", label: "打开", icon: <PlayCircleOutlined /> },
                {
                  key: "reveal",
                  label: "访达中显示",
                  icon: <FolderOpenOutlined />,
                  disabled: !isPathLocator,
                },
                {
                  key: "open_in_terminal",
                  label: "在终端中打开",
                  icon: <CodeOutlined />,
                  disabled: !isPathLocator,
                },
                {
                  key: "copy_path",
                  label: "复制路径",
                  icon: <CopyOutlined />,
                  disabled: !isPathLocator,
                },
                { type: "divider" as const },
                {
                  key: "open_with",
                  label: "用其他程序打开…",
                  icon: <ExportOutlined />,
                  disabled: !isPathLocator,
                },
              ];
              const handleMenuClick = ({ key }: { key: string }) => {
                if (key === "open") void handleOpen(ref);
                else if (key === "reveal") void handleReveal(ref);
                else if (key === "open_in_terminal") void handleOpenInTerminal(ref);
                else if (key === "copy_path") void handleCopyPath(ref);
                else if (key === "open_with") void handleOpenWith(ref);
              };
              return (
                <Dropdown
                  key={ref.id}
                  menu={{ items: contextMenuItems, onClick: handleMenuClick }}
                  trigger={["contextMenu"]}
                >
                  <div
                    onDoubleClick={() => {
                      if (isPathLocator) void handleOpen(ref);
                    }}
                    style={{ cursor: isPathLocator ? "pointer" : "default" }}
                  >
                    <List.Item
                      actions={[
                        <Button
                          key="open"
                          type="text"
                          size="small"
                          icon={<PlayCircleOutlined />}
                          disabled={!isPathLocator}
                          onClick={() => void handleOpen(ref)}
                        >
                          打开
                        </Button>,
                        <Button
                          key="edit"
                          type="text"
                          size="small"
                          icon={<EditOutlined />}
                          onClick={() => openEditModal(ref)}
                        >
                          编辑
                        </Button>,
                        ...(canShowUndoButton(ref)
                          ? [
                              <Button
                                key="undo"
                                type="text"
                                size="small"
                                icon={<RollbackOutlined />}
                                onClick={() => openUndoDialog(ref)}
                              >
                                撤销导入
                              </Button>,
                            ]
                          : []),
                        <DispositionButtons
                          key="disposition"
                          reference={ref}
                          size="small"
                          onAction={(action) => openDispositionDialog(ref, action)}
                        />,
                      ]}
                    >
                      <List.Item.Meta
                        title={
                          <AntSpace size={8} wrap>
                            <span style={{ fontWeight: 600 }}>{ref.name}</span>
                            <Badge status={meta.color as "success" | "error" | "default"} text={meta.text} />
                            <Tag>{LIFECYCLE_LABEL[ref.lifecycle] ?? ref.lifecycle}</Tag>
                            <Tag color="blue">
                              {CONFIDENTIALITY_LABEL[ref.confidentiality] ?? ref.confidentiality}
                            </Tag>
                          </AntSpace>
                        }
                        description={
                          <AntSpace direction="vertical" size={2} style={{ width: "100%" }}>
                            <Text type="secondary" style={{ fontFamily: "monospace", fontSize: 12 }}>
                              {locatorText(ref)}
                            </Text>
                            <Text type="secondary" style={{ fontSize: 12 }}>
                              创建时间：{formatUnixSeconds(ref.createdAt)}
                            </Text>
                          </AntSpace>
                        }
                      />
                    </List.Item>
                  </div>
                </Dropdown>
              );
            }}
          />
        ),
    };
  });

  return (
    <div style={{ maxWidth: 1080 }}>
      {messageContextHolder}

      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          marginBottom: 16,
        }}
      >
        <AntSpace>
          <Button icon={<ArrowLeftOutlined />} onClick={onBack}>
            返回资源集
          </Button>
          <h1 style={{ fontSize: 20, margin: 0 }}>
            {space.name} · {collection.name}
          </h1>
        </AntSpace>
        <AntSpace>
          <Dropdown
            menu={{
              items: [
                { key: "external", label: "仅关联（不改动原文件）" },
                { key: "managed", label: "导入并托管（复制 / 移动到根目录）" },
              ],
              onClick: ({ key }) => {
                if (key === "external") setCreateOpen(true);
                if (key === "managed") setManagedOpen(true);
              },
            }}
            disabled={detail?.status === "archived"}
          >
            <Button type="primary" icon={<PlusOutlined />}>
              <AntSpace size={4}>
                添加引用
                <DownOutlined style={{ fontSize: 10 }} />
              </AntSpace>
            </Button>
          </Dropdown>
          <Button icon={<ReloadOutlined />} onClick={fetchDetail} loading={loading}>
            刷新
          </Button>
        </AntSpace>
      </div>

      {loading && !detail ? (
        <div style={{ textAlign: "center", padding: "48px 0" }}>
          <Spin size="large" />
        </div>
      ) : detail ? (
        <>
          <Descriptions
            bordered
            size="small"
            column={2}
            style={{ marginBottom: 16 }}
            items={[
              {
                key: "name",
                label: "名称",
                children: <span style={{ fontWeight: 600 }}>{detail.name}</span>,
              },
              {
                key: "status",
                label: "状态",
                children:
                  detail.status === "active" ? (
                    <Tag color="success">活跃</Tag>
                  ) : (
                    <Tag color="warning">已归档</Tag>
                  ),
              },
              {
                key: "summary",
                label: "简介",
                span: 2,
                children: detail.summary ? (
                  <Paragraph style={{ margin: 0 }}>{detail.summary}</Paragraph>
                ) : (
                  <Text type="secondary">—</Text>
                ),
              },
              {
                key: "tags",
                label: "标签",
                span: 2,
                children:
                  detail.tags && detail.tags.length > 0 ? (
                    <AntSpace size={4} wrap>
                      {detail.tags.map((t) => (
                        <Tag key={t}>{t}</Tag>
                      ))}
                    </AntSpace>
                  ) : (
                    <Text type="secondary">—</Text>
                  ),
              },
              {
                key: "createdAt",
                label: "创建时间",
                children: formatUnixSeconds(detail.createdAt),
              },
              {
                key: "updatedAt",
                label: "更新时间",
                children: formatUnixSeconds(detail.updatedAt),
              },
            ]}
          />

          <Collapse
            items={collapseItems}
            defaultActiveKey={TYPE_ORDER.filter(
              (t) => (detail.referencesByType[t] ?? []).length > 0,
            )}
          />
        </>
      ) : (
        <Empty description="未加载到资源集详情" />
      )}

      <ReferenceCreateDialog
        open={createOpen}
        collectionId={collection.id}
        onClose={() => setCreateOpen(false)}
        onCreated={fetchDetail}
      />
      <ReferenceManagedDialog
        open={managedOpen}
        collectionId={collection.id}
        onClose={() => setManagedOpen(false)}
        onCreated={fetchDetail}
      />
      <DispositionConfirmDialog
        action={dispositionAction}
        reference={dispositionRef}
        onClose={closeDispositionDialog}
        onSuccess={fetchDetail}
      />
      <UndoImportDialog
        reference={undoRef}
        onClose={closeUndoDialog}
        onSuccess={fetchDetail}
      />
      <Modal
        title="编辑引用"
        open={editingRef !== null}
        onOk={handleSave}
        onCancel={closeEditModal}
        confirmLoading={saving}
        okText="保存"
        cancelText="取消"
        forceRender
        maskClosable={false}
      >
        {editingRef && (
          <div style={{ marginBottom: 12 }}>
            <Text type="secondary" style={{ fontSize: 12 }}>
              类型与定位不可修改：
              {TYPE_LABEL[editingRef.type]} · {locatorText(editingRef)}
            </Text>
          </div>
        )}
        <Form form={form} layout="vertical" preserve={false}>
          <Form.Item
            name="name"
            label="名称"
            rules={[
              { required: true, message: "请输入名称" },
              { whitespace: true, message: "名称不能为空" },
            ]}
          >
            <Input placeholder="引用名称" maxLength={120} />
          </Form.Item>
          <Form.Item name="description" label="描述">
            <Input.TextArea rows={3} placeholder="可选描述" maxLength={500} />
          </Form.Item>
          <Form.Item name="tags" label="标签">
            <Select mode="tags" placeholder="输入后回车添加标签" tokenSeparators={[","]} />
          </Form.Item>
          <Form.Item name="lifecycle" label="生命周期" rules={[{ required: true }]}>
            <Select options={LIFECYCLE_OPTIONS} />
          </Form.Item>
          <Form.Item name="confidentiality" label="保密级别" rules={[{ required: true }]}>
            <Select options={CONFIDENTIALITY_OPTIONS} />
          </Form.Item>
          <Form.Item name="indexed" label="纳入索引" valuePropName="checked">
            <Switch />
          </Form.Item>
        </Form>
        {/* M7-2 · 关联待办：列出挂载的 todo + 提供"挂到待办"入口 */}
        {editingRef && (
          <div style={{ marginTop: 16, borderTop: "1px solid #f0f0f0", paddingTop: 16 }}>
            <ReferenceTodoPanel reference={editingRef} onChanged={fetchDetail} />
          </div>
        )}
      </Modal>
    </div>
  );
}
