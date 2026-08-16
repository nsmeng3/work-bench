import { useEffect, useMemo, useState } from "react";
import {
  Alert,
  Button,
  Descriptions,
  Form,
  Input,
  Modal,
  Radio,
  Select,
  Space as AntSpace,
  Typography,
  message,
} from "antd";
import type {
  Collection,
  InboxItem,
  InboxAssignInput,
  ManagedAction,
  ManagedPlan,
  ReferenceType,
  Space,
} from "../api";
import {
  collectionList,
  inboxAssign,
  inboxDismissStale,
  spaceList,
  toApiError,
} from "../api";

const { Text } = Typography;

/**
 * 收件箱条目「处理」对话框 — 任务包 m5-5.7。
 * 契约：详细设计说明书 §2.7 inbox_assign / inbox_dismiss_stale。
 *
 * 流程：
 * - 阶段一（edit）：选目标空间/资源集/类型 + 处理方式（external/managed）
 *   - external：直接调 inbox_assign { mode: 'external', confirmed: true }
 *   - managed：先调 inbox_assign { mode: 'managed', confirmed: false } 拿 managedPlan
 * - 阶段二（confirm）：展示 plan，允许改 targetName / managedAction，二次确认后调
 *   inbox_assign { mode: 'managed', confirmed: true }
 *
 * 错误处理：
 * - INBOX_STALE：提示「源文件已不在」+ 引导「标记为已处理」（调 inbox_dismiss_stale）
 * - FS_TARGET_EXISTS：提示目标已存在，引导改名
 * - 其他按统一错误模型 message
 */

const TYPE_OPTIONS: { value: ReferenceType; label: string }[] = [
  { value: "code", label: "代码" },
  { value: "document", label: "文档" },
  { value: "data", label: "数据" },
  { value: "artifact", label: "构建产物" },
  { value: "tool", label: "工具" },
  { value: "media", label: "媒体" },
];

function formatBytes(n: number): string {
  if (!Number.isFinite(n) || n < 0) return "-";
  if (n < 1024) return `${n} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let v = n;
  let u = -1;
  do {
    v /= 1024;
    u += 1;
  } while (v >= 1024 && u < units.length - 1);
  return `${v.toFixed(v >= 100 ? 0 : v >= 10 ? 1 : 2)} ${units[u]}`;
}

interface InboxAssignDialogProps {
  open: boolean;
  /** 当前处理的收件箱条目 */
  item: InboxItem | null;
  onClose: () => void;
  /** 处理成功后回调（父组件刷新列表 + 角标） */
  onAssigned: () => void;
}

interface EditFormValues {
  mode: "external" | "managed";
  spaceId: string;
  collectionId: string;
  type: ReferenceType;
  targetName?: string;
  managedAction: ManagedAction;
}

type Stage = "edit" | "confirm";

export function InboxAssignDialog({
  open,
  item,
  onClose,
  onAssigned,
}: InboxAssignDialogProps) {
  const [form] = Form.useForm<EditFormValues>();
  const [messageApi, contextHolder] = message.useMessage();

  const [stage, setStage] = useState<Stage>("edit");
  const [spaces, setSpaces] = useState<Space[]>([]);
  const [collections, setCollections] = useState<Collection[]>([]);
  const [spacesLoading, setSpacesLoading] = useState(false);
  const [collectionsLoading, setCollectionsLoading] = useState(false);
  const [submitLoading, setSubmitLoading] = useState(false);
  const [plan, setPlan] = useState<ManagedPlan | null>(null);
  const [editValues, setEditValues] = useState<EditFormValues | null>(null);

  // 确认框本地状态
  const [confirmTargetName, setConfirmTargetName] = useState("");
  const [confirmAction, setConfirmAction] = useState<ManagedAction>("copy");

  // INBOX_STALE 引导
  const [staleModalOpen, setStaleModalOpen] = useState(false);
  const [staleDismissing, setStaleDismissing] = useState(false);

  const selectedSpaceId = Form.useWatch("spaceId", form);
  const selectedMode = Form.useWatch("mode", form);

  // 打开时加载空间列表 + 重置
  useEffect(() => {
    if (!open || !item) return;
    setStage("edit");
    setPlan(null);
    setEditValues(null);
    setConfirmTargetName("");
    setConfirmAction("copy");
    setStaleModalOpen(false);

    form.setFieldsValue({
      mode: "external",
      spaceId: undefined,
      collectionId: undefined,
      type: item.suggestedType ?? "document",
      targetName: undefined,
      managedAction: "copy",
    });

    setSpacesLoading(true);
    spaceList({ status: "active" })
      .then((list) => {
        setSpaces(list);
        // 若只有一个空间，自动选中
        if (list.length === 1) {
          form.setFieldsValue({ spaceId: list[0].id });
        }
      })
      .catch((err) => {
        const apiErr = toApiError(err);
        messageApi.error(`加载空间列表失败：${apiErr.message}`);
      })
      .finally(() => setSpacesLoading(false));
  }, [open, item, form, messageApi]);

  // 空间变化时加载资源集
  useEffect(() => {
    if (!selectedSpaceId) {
      setCollections([]);
      form.setFieldsValue({ collectionId: undefined });
      return;
    }
    setCollectionsLoading(true);
    collectionList({ spaceId: selectedSpaceId, status: "active" })
      .then((list) => {
        setCollections(list);
        // 若当前选中的 collectionId 不在新列表中，清空
        const current = form.getFieldValue("collectionId") as string | undefined;
        if (current && !list.some((c) => c.id === current)) {
          form.setFieldsValue({ collectionId: undefined });
        }
        // 若只有一个资源集，自动选中
        if (list.length === 1) {
          form.setFieldsValue({ collectionId: list[0].id });
        }
      })
      .catch((err) => {
        const apiErr = toApiError(err);
        messageApi.error(`加载资源集失败：${apiErr.message}`);
      })
      .finally(() => setCollectionsLoading(false));
  }, [selectedSpaceId, form, messageApi]);

  /** 目标路径预览（前端本地拼接） */
  const targetPreview = useMemo(() => {
    if (!plan) return "";
    const current = confirmTargetName.trim();
    if (!current) return plan.proposedTarget;
    const idx = plan.proposedTarget.lastIndexOf("/");
    if (idx < 0) return plan.proposedTarget;
    return `${plan.proposedTarget.slice(0, idx + 1)}${current}`;
  }, [plan, confirmTargetName]);

  /** 前端本地冲突检查 */
  const activeConflicts = useMemo(() => {
    if (!plan) return [] as string[];
    if (plan.conflicts.length === 0) return [] as string[];
    const current = confirmTargetName.trim();
    if (!current) return plan.conflicts;
    return plan.conflicts.filter((c) => c.includes(current));
  }, [plan, confirmTargetName]);

  /** 处理 INBOX_STALE 错误 */
  function handleStaleError() {
    setStaleModalOpen(true);
  }

  /** 标记为已处理（dismiss stale） */
  async function handleDismissStale() {
    if (!item) return;
    setStaleDismissing(true);
    try {
      await inboxDismissStale({ id: item.id });
      messageApi.success("已标记为已处理");
      setStaleModalOpen(false);
      onAssigned();
      onClose();
    } catch (err) {
      const apiErr = toApiError(err);
      messageApi.error(`操作失败：${apiErr.message}`);
    } finally {
      setStaleDismissing(false);
    }
  }

  /** 阶段一提交 */
  async function handleEditSubmit() {
    if (!item) return;
    const values = await form.validateFields();
    const trimmedTargetName = values.targetName?.trim() || undefined;

    if (values.mode === "external") {
      // external：直接 confirmed=true
      const input: InboxAssignInput = {
        id: item.id,
        mode: "external",
        spaceId: values.spaceId,
        collectionId: values.collectionId,
        type: values.type,
        confirmed: true,
      };
      setSubmitLoading(true);
      try {
        await inboxAssign(input);
        messageApi.success("已转为正式引用");
        onAssigned();
        onClose();
      } catch (err) {
        const apiErr = toApiError(err);
        if (apiErr.code === "INBOX_STALE") {
          handleStaleError();
        } else {
          messageApi.error(
            apiErr.retryable ? `${apiErr.message}（可重试）` : apiErr.message,
          );
        }
      } finally {
        setSubmitLoading(false);
      }
    } else {
      // managed：先拿 plan
      const input: InboxAssignInput = {
        id: item.id,
        mode: "managed",
        spaceId: values.spaceId,
        collectionId: values.collectionId,
        type: values.type,
        managedAction: values.managedAction,
        confirmed: false,
      };
      setSubmitLoading(true);
      try {
        const result = await inboxAssign(input);
        if (result.managedPlan) {
          setPlan(result.managedPlan);
          setEditValues({ ...values, targetName: trimmedTargetName });
          setConfirmAction(values.managedAction);
          const segments = result.managedPlan.proposedTarget.split("/").filter(Boolean);
          setConfirmTargetName(trimmedTargetName ?? segments[segments.length - 1] ?? "");
          setStage("confirm");
        }
      } catch (err) {
        const apiErr = toApiError(err);
        if (apiErr.code === "INBOX_STALE") {
          handleStaleError();
        } else {
          messageApi.error(
            apiErr.retryable ? `${apiErr.message}（可重试）` : apiErr.message,
          );
        }
      } finally {
        setSubmitLoading(false);
      }
    }
  }

  /** 阶段二确认（managed confirmed=true） */
  async function handleConfirm() {
    if (!item || !plan || !editValues) return;
    if (activeConflicts.length > 0) return;

    const input: InboxAssignInput = {
      id: item.id,
      mode: "managed",
      spaceId: editValues.spaceId,
      collectionId: editValues.collectionId,
      type: editValues.type,
      managedAction: confirmAction,
      targetName: confirmTargetName.trim() || undefined,
      confirmed: true,
    };

    setSubmitLoading(true);
    try {
      await inboxAssign(input);
      messageApi.success("已转为正式引用");
      onAssigned();
      onClose();
    } catch (err) {
      const apiErr = toApiError(err);
      if (apiErr.code === "INBOX_STALE") {
        handleStaleError();
      } else if (apiErr.code === "FS_TARGET_EXISTS") {
        messageApi.error(`目标已存在：${apiErr.message}。请修改目标名后重试。`);
      } else {
        messageApi.error(
          apiErr.retryable ? `${apiErr.message}（可重试）` : apiErr.message,
        );
      }
    } finally {
      setSubmitLoading(false);
    }
  }

  function handleCancel() {
    if (submitLoading) return;
    onClose();
  }

  function handleBack() {
    if (submitLoading) return;
    setStage("edit");
  }

  const baseName = item?.path.split("/").filter(Boolean).pop() ?? "";

  return (
    <>
      {contextHolder}
      <Modal
        title={stage === "edit" ? "处理收件箱条目" : "导入并托管 · 确认"}
        open={open}
        onCancel={handleCancel}
        width={640}
        destroyOnHidden
        maskClosable={false}
        footer={
          stage === "edit" ? (
            <>
              <Button onClick={handleCancel}>取消</Button>
              <Button
                type="primary"
                onClick={() => void handleEditSubmit()}
                loading={submitLoading}
              >
                {selectedMode === "external" ? "确认" : "下一步"}
              </Button>
            </>
          ) : (
            <>
              <Button onClick={handleBack} disabled={submitLoading}>
                上一步
              </Button>
              <Button onClick={handleCancel}>取消</Button>
              <Button
                type="primary"
                onClick={() => void handleConfirm()}
                loading={submitLoading}
                disabled={activeConflicts.length > 0}
              >
                确认导入
              </Button>
            </>
          )
        }
      >
        {stage === "edit" ? (
          <>
            {/* 条目信息只读展示 */}
            <Descriptions
              bordered
              size="small"
              column={1}
              style={{ marginBottom: 16 }}
              items={[
                {
                  key: "name",
                  label: "文件名",
                  children: <Text strong>{baseName}</Text>,
                },
                {
                  key: "path",
                  label: "路径",
                  children: (
                    <Text code style={{ fontSize: 12 }}>
                      {item?.path}
                    </Text>
                  ),
                },
                {
                  key: "suggestedType",
                  label: "建议类型",
                  children: item?.suggestedType
                    ? (TYPE_OPTIONS.find((t) => t.value === item.suggestedType)?.label ??
                      item.suggestedType)
                    : "未识别",
                },
              ]}
            />

            <Form form={form} layout="vertical" preserve={false}>
              <Form.Item
                name="mode"
                label="处理方式"
                rules={[{ required: true, message: "请选择处理方式" }]}
              >
                <Radio.Group>
                  <Radio value="external">仅关联（文件不动）</Radio>
                  <Radio value="managed">导入并托管（复制/移动到根目录）</Radio>
                </Radio.Group>
              </Form.Item>

              <Form.Item
                name="spaceId"
                label="目标空间"
                rules={[{ required: true, message: "请选择目标空间" }]}
              >
                <Select
                  placeholder="选择空间"
                  loading={spacesLoading}
                  options={spaces.map((s) => ({ value: s.id, label: s.name }))}
                />
              </Form.Item>

              <Form.Item
                name="collectionId"
                label="目标资源集"
                rules={[{ required: true, message: "请选择目标资源集" }]}
              >
                <Select
                  placeholder={selectedSpaceId ? "选择资源集" : "请先选择空间"}
                  loading={collectionsLoading}
                  disabled={!selectedSpaceId}
                  options={collections.map((c) => ({ value: c.id, label: c.name }))}
                />
              </Form.Item>

              <Form.Item
                name="type"
                label="类型"
                rules={[{ required: true, message: "请选择类型" }]}
              >
                <Select options={TYPE_OPTIONS} />
              </Form.Item>

              {selectedMode === "managed" && (
                <>
                  <Form.Item
                    name="targetName"
                    label="目标名（可选）"
                    extra={
                      <Text type="secondary" style={{ fontSize: 12 }}>
                        缺省使用源文件名：{baseName}
                      </Text>
                    }
                  >
                    <Input placeholder={baseName} allowClear />
                  </Form.Item>

                  <Form.Item
                    name="managedAction"
                    label="托管动作"
                    rules={[{ required: true }]}
                  >
                    <Radio.Group>
                      <Radio value="copy">复制（保留源文件）</Radio>
                      <Radio value="move">移动（源文件将被删除）</Radio>
                    </Radio.Group>
                  </Form.Item>
                </>
              )}
            </Form>
          </>
        ) : plan ? (
          <>
            <Descriptions
              bordered
              size="small"
              column={1}
              style={{ marginBottom: 16 }}
              items={[
                {
                  key: "source",
                  label: "源路径",
                  children: (
                    <Text style={{ fontFamily: "monospace", fontSize: 12 }}>
                      {plan.source}
                    </Text>
                  ),
                },
                {
                  key: "target",
                  label: "目标路径",
                  children: (
                    <Text style={{ fontFamily: "monospace", fontSize: 12 }}>
                      {targetPreview}
                    </Text>
                  ),
                },
                {
                  key: "size",
                  label: "大小",
                  children: (
                    <AntSpace size={8}>
                      <span>{formatBytes(plan.sizeBytes)}</span>
                      {plan.fileCount > 1 && (
                        <Text type="secondary" style={{ fontSize: 12 }}>
                          共 {plan.fileCount} 个文件
                        </Text>
                      )}
                    </AntSpace>
                  ),
                },
              ]}
            />

            <Form layout="vertical">
              <Form.Item
                label="自定义目标名"
                extra={
                  <Text type="secondary" style={{ fontSize: 12 }}>
                    修改后目标路径实时预览；不会再次请求后端
                  </Text>
                }
                validateStatus={activeConflicts.length > 0 ? "warning" : undefined}
                help={
                  activeConflicts.length > 0 ? "目标名仍与既有项冲突，请更换" : undefined
                }
              >
                <Input
                  value={confirmTargetName}
                  onChange={(e) => setConfirmTargetName(e.target.value)}
                  placeholder="目标名（缺省用源名）"
                  allowClear
                />
              </Form.Item>

              <Form.Item label="托管动作">
                <Radio.Group
                  value={confirmAction}
                  onChange={(e) => setConfirmAction(e.target.value as ManagedAction)}
                >
                  <Radio value="copy">复制（保留源文件）</Radio>
                  <Radio value="move">移动（源文件将被删除）</Radio>
                </Radio.Group>
              </Form.Item>
            </Form>

            {plan.conflicts.length > 0 && (
              <Alert
                type="warning"
                showIcon
                message="检测到目标冲突"
                description={
                  <ul style={{ margin: 0, paddingLeft: 18 }}>
                    {plan.conflicts.map((c, i) => (
                      <li key={i}>{c}</li>
                    ))}
                  </ul>
                }
                style={{ marginBottom: 12 }}
              />
            )}

            {confirmAction === "move" && (
              <Alert
                type="info"
                showIcon
                message="移动动作将删除源文件"
                description="确认后源路径下的文件将被移动到目标位置，此操作不可撤销。"
              />
            )}
          </>
        ) : null}
      </Modal>

      {/* INBOX_STALE 引导对话框 */}
      <Modal
        title="源文件已不在"
        open={staleModalOpen}
        onCancel={() => setStaleModalOpen(false)}
        footer={
          <>
            <Button onClick={() => setStaleModalOpen(false)}>取消</Button>
            <Button
              type="primary"
              onClick={() => void handleDismissStale()}
              loading={staleDismissing}
            >
              标记为已处理
            </Button>
          </>
        }
      >
        <Text>
          源文件已不在原路径，无法继续处理。你可以将此条目标记为已处理（不再跟踪）。
        </Text>
      </Modal>
    </>
  );
}
