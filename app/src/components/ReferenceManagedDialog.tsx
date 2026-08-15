import { useEffect, useMemo, useRef, useState } from "react";
import {
  Alert,
  Button,
  Descriptions,
  Form,
  Input,
  Modal,
  Progress,
  Radio,
  Space as AntSpace,
  Typography,
  message,
} from "antd";
import { FileOutlined, FolderOpenOutlined } from "@ant-design/icons";
import type {
  ManagedAction,
  ManagedPlan,
  RefCreateManagedInput,
  ReferenceConfidentiality,
  ReferenceLifecycle,
  ReferenceType,
} from "../api";
import { refCreateManaged, toApiError } from "../api";
import { ReferenceBaseFields } from "./ReferenceBaseFields";
import { useManagedProgress } from "../hooks/useManagedProgress";

const { Text } = Typography;

/**
 * 将字节数格式化为人类可读字符串（KB / MB / GB）。
 */
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

/**
 * 通过 Tauri dialog 选择文件/目录。
 * 与 ReferenceCreateDialog 中的实现一致；插件未注册时返回 null 由调用方提示手输。
 */
async function pickPath(kind: "file" | "directory"): Promise<string | null> {
  try {
    const dialog = await import("@tauri-apps/plugin-dialog");
    const selected = await dialog.open({
      directory: kind === "directory",
      multiple: false,
    });
    if (typeof selected === "string") return selected;
    return null;
  } catch (err) {
    console.warn("tauri dialog 不可用，降级为手动输入：", err);
    return null;
  }
}

/**
 * 生成临时 UUID —— 3.7 hook 用。
 * 优先使用 crypto.randomUUID；老环境降级为简单随机串。
 */
function makeTempRefId(): string {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID();
  }
  return `tmp-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`;
}

interface ReferenceManagedDialogProps {
  open: boolean;
  /** 所属资源集 id */
  collectionId: string;
  onClose: () => void;
  /** 创建成功后回调（父组件应重新拉取 collection_get） */
  onCreated: () => void;
  /**
   * 3.7 hook：confirmed 调用前生成临时 refId 并通过此回调暴露。
   * 3.7 任务将用该 id 订阅 `managed_progress` 事件做进度展示。
   * 本任务内部已生成临时 id 并打日志，父组件可选择性接收。
   */
  onRefIdReady?: (tempRefId: string) => void;
}

interface StageOneFormValues {
  path: string;
  type: ReferenceType;
  name: string;
  description?: string;
  tags?: string[];
  lifecycle: ReferenceLifecycle;
  confidentiality: ReferenceConfidentiality;
}

type Stage = "edit" | "confirm";

/**
 * 导入并托管 — 两阶段确认对话框（§2.5 ref_create_managed / §4.1）。
 *
 * 阶段一（edit）：填写基础字段 + 源路径，点击「下一步」调 confirmed=false 拿 ManagedPlan。
 * 阶段二（confirm）：展示 plan，允许自定义 targetName 与 managedAction；
 *   conflicts 非空时禁用「确认」直至用户改 targetName 使冲突消失（前端本地检查）。
 * 任何阶段取消都直接关闭，**不产生任何写操作**。
 */
export function ReferenceManagedDialog({
  open,
  collectionId,
  onClose,
  onCreated,
  onRefIdReady,
}: ReferenceManagedDialogProps) {
  const [form] = Form.useForm<StageOneFormValues>();
  const [messageApi, contextHolder] = message.useMessage();

  const [stage, setStage] = useState<Stage>("edit");
  const [planLoading, setPlanLoading] = useState(false);
  const [confirmLoading, setConfirmLoading] = useState(false);
  const [plan, setPlan] = useState<ManagedPlan | null>(null);
  const [stageOneValues, setStageOneValues] = useState<StageOneFormValues | null>(null);

  // 确认框内的本地状态
  const [targetName, setTargetName] = useState<string>("");
  const [managedAction, setManagedAction] = useState<ManagedAction>("copy");

  // m3-3.7 进度展示：当前 confirmed 任务的临时 refId 与失败标记
  const [currentRefId, setCurrentRefId] = useState<string | null>(null);
  const [progressFailed, setProgressFailed] = useState(false);
  // 用于失败"短暂停留后关闭"的定时器
  const failureTimerRef = useRef<number | null>(null);
  const { get: getProgress, clear: clearProgress } = useManagedProgress();
  const currentProgress = getProgress(currentRefId);

  // 打开时重置
  useEffect(() => {
    if (open) {
      setStage("edit");
      setPlan(null);
      setStageOneValues(null);
      setTargetName("");
      setManagedAction("copy");
      setCurrentRefId(null);
      setProgressFailed(false);
      form.setFieldsValue({
        path: "",
        type: "document",
        name: "",
        description: "",
        tags: [],
        lifecycle: "active",
        confidentiality: "internal",
      });
    } else {
      form.resetFields();
    }
  }, [open, form]);

  // 卸载时清理失败定时器
  useEffect(() => {
    return () => {
      if (failureTimerRef.current !== null) {
        window.clearTimeout(failureTimerRef.current);
        failureTimerRef.current = null;
      }
    };
  }, []);

  async function handlePick(kind: "file" | "directory") {
    const path = await pickPath(kind);
    if (path) {
      form.setFieldsValue({ path });
      const currentName = form.getFieldValue("name") as string | undefined;
      if (!currentName || currentName.trim() === "") {
        const segments = path.split("/").filter(Boolean);
        const base = segments[segments.length - 1] ?? path;
        form.setFieldsValue({ name: base });
      }
    } else {
      messageApi.info("系统文件选择器不可用，请手动输入绝对路径");
    }
  }

  /** 阶段一 → 调 plan */
  async function handleNext() {
    const values = await form.validateFields();
    const pathTrimmed = values.path.trim();
    const nameTrimmed = values.name.trim();

    const input: RefCreateManagedInput = {
      collectionId,
      name: nameTrimmed,
      type: values.type,
      locator: { kind: "path", path: pathTrimmed },
      description: values.description?.trim() || undefined,
      tags: values.tags && values.tags.length > 0 ? values.tags : undefined,
      lifecycle: values.lifecycle,
      confidentiality: values.confidentiality,
      managedAction: "copy", // 默认 copy；用户可在确认框改
      confirmed: false,
    };

    setPlanLoading(true);
    try {
      const p = await refCreateManaged({ ...input, confirmed: false });
      setPlan(p);
      setStageOneValues({ ...values, path: pathTrimmed, name: nameTrimmed });
      setManagedAction(p.action);
      // 默认 targetName 用 proposedTarget 的末段
      const segments = p.proposedTarget.split("/").filter(Boolean);
      setTargetName(segments[segments.length - 1] ?? "");
      setStage("confirm");
    } catch (err) {
      const apiErr = toApiError(err);
      if (apiErr.code === "FS_PATH_NOT_FOUND") {
        messageApi.error(`源路径不存在：${pathTrimmed}。请检查路径后重试。`);
      } else if (apiErr.code === "COMMON_IO") {
        messageApi.error(`IO 错误：${apiErr.message}`);
      } else {
        messageApi.error(apiErr.retryable ? `${apiErr.message}（可重试）` : apiErr.message);
      }
    } finally {
      setPlanLoading(false);
    }
  }

  /**
   * 前端本地冲突检查：根据 plan.conflicts 与当前 targetName 计算仍存在哪些冲突。
   *
   * 契约说明：后端在 plan 阶段返回的 conflicts 描述的是「默认 targetName（源名）」的冲突。
   * 用户改 targetName 后，前端做字符串级匹配 —— 若所有冲突条目中引用的目标名都不再等于
   * 当前 targetName，则认为冲突已消除。这是简化实现，3.7+ 可考虑改为后端实时校验。
   */
  const activeConflicts = useMemo(() => {
    if (!plan) return [] as string[];
    if (plan.conflicts.length === 0) return [] as string[];
    const current = targetName.trim();
    if (!current) return plan.conflicts;
    // 若冲突文本中包含当前 targetName，则视为仍存在；否则视为已规避
    return plan.conflicts.filter((c) => c.includes(current));
  }, [plan, targetName]);

  /** 目标路径预览（前端本地拼接，不再次调 plan） */
  const targetPreview = useMemo(() => {
    if (!plan) return "";
    const current = targetName.trim();
    if (!current) return plan.proposedTarget;
    const idx = plan.proposedTarget.lastIndexOf("/");
    if (idx < 0) return plan.proposedTarget;
    return `${plan.proposedTarget.slice(0, idx + 1)}${current}`;
  }, [plan, targetName]);

  /** 阶段二 → confirmed=true */
  async function handleConfirm() {
    if (!plan || !stageOneValues) return;
    if (activeConflicts.length > 0) return; // 防御：按钮已禁用

    // m3-3.7：生成临时 refId 并订阅 managed_progress 事件做进度展示
    const tempRefId = makeTempRefId();
    setCurrentRefId(tempRefId);
    setProgressFailed(false);
    onRefIdReady?.(tempRefId);
    // eslint-disable-next-line no-console
    console.log("[m3-3.7] confirmed 调用临时 refId（订阅 managed_progress）:", tempRefId);

    const input: RefCreateManagedInput = {
      collectionId,
      name: stageOneValues.name,
      type: stageOneValues.type,
      locator: { kind: "path", path: stageOneValues.path },
      description: stageOneValues.description?.trim() || undefined,
      tags:
        stageOneValues.tags && stageOneValues.tags.length > 0
          ? stageOneValues.tags
          : undefined,
      lifecycle: stageOneValues.lifecycle,
      confidentiality: stageOneValues.confidentiality,
      managedAction,
      targetName: targetName.trim() || undefined,
      confirmed: true,
    };

    setConfirmLoading(true);
    try {
      await refCreateManaged({ ...input, confirmed: true });
      messageApi.success(`已导入并托管到 ${targetPreview}`);
      clearProgress(tempRefId);
      setCurrentRefId(null);
      onCreated();
      onClose();
    } catch (err) {
      const apiErr = toApiError(err);
      // m3-3.7 失败处理（固化选择）：进度条 status="exception" 短暂停留（1.5s）后关闭确认框。
      // 备选方案是"留在确认框允许重试"，本任务选择前者，避免用户在失败态下误点重试
      // 导致后端冲突；用户可重新打开对话框再次尝试。
      setProgressFailed(true);
      if (failureTimerRef.current !== null) {
        window.clearTimeout(failureTimerRef.current);
      }
      failureTimerRef.current = window.setTimeout(() => {
        failureTimerRef.current = null;
        clearProgress(tempRefId);
        setCurrentRefId(null);
        setProgressFailed(false);
        onClose();
      }, 1500);

      if (apiErr.code === "FS_TARGET_EXISTS") {
        messageApi.error(`目标已存在：${apiErr.message}。请修改目标名后重试。`);
      } else if (apiErr.code === "FS_PATH_NOT_FOUND") {
        messageApi.error(`源路径已不存在：${apiErr.message}`);
      } else if (apiErr.code === "COMMON_IO") {
        messageApi.error(`IO 错误：${apiErr.message}`);
      } else {
        messageApi.error(apiErr.retryable ? `${apiErr.message}（可重试）` : apiErr.message);
      }
    } finally {
      setConfirmLoading(false);
    }
  }

  function handleCancel() {
    // 任何阶段取消都直接关闭，无写操作
    if (planLoading) return;
    // m3-3.7 已知限制（契约冻结）：confirmed 进行中允许取消，但**不真正中断后端复制**。
    // 后端无取消机制（详见任务包 m3-3.7 §5）；此处仅 UI 隐藏进度，后台复制继续完成。
    // 用户重新打开对话框时不会看到上次的进度（currentRefId 已重置）。
    if (confirmLoading) {
      if (currentRefId) {
        clearProgress(currentRefId);
      }
      setCurrentRefId(null);
      setProgressFailed(false);
      setConfirmLoading(false);
      onClose();
      return;
    }
    onClose();
  }

  function handleBack() {
    if (confirmLoading) return;
    setStage("edit");
  }

  return (
    <>
      {contextHolder}
      <Modal
        title={stage === "edit" ? "导入并托管 · 步骤 1/2" : "导入并托管 · 步骤 2/2"}
        open={open}
        onCancel={handleCancel}
        width={640}
        destroyOnHidden
        maskClosable={false}
        footer={
          stage === "edit" ? (
            <>
              <Button onClick={handleCancel}>取消</Button>
              <Button type="primary" onClick={handleNext} loading={planLoading}>
                下一步
              </Button>
            </>
          ) : (
            <>
              <Button onClick={handleBack} disabled={confirmLoading}>
                上一步
              </Button>
              {/* m3-3.7：confirmed 进行中允许取消（仅 UI 隐藏进度，后端继续复制 —— 已知限制） */}
              <Button onClick={handleCancel}>
                取消
              </Button>
              <Button
                type="primary"
                onClick={handleConfirm}
                loading={confirmLoading}
                disabled={activeConflicts.length > 0}
              >
                确认导入
              </Button>
            </>
          )
        }
      >
        {stage === "edit" ? (
          <Form form={form} layout="vertical" preserve={false}>
            <Form.Item
              name="path"
              label="源文件 / 目录路径"
              rules={[
                { required: true, whitespace: true, message: "路径不能为空" },
                {
                  validator: (_, v: string) =>
                    v && v.trim().startsWith("/")
                      ? Promise.resolve()
                      : Promise.reject(new Error("请输入绝对路径（以 / 开头）")),
                },
              ]}
              extra={
                <Text type="secondary" style={{ fontSize: 12 }}>
                  将按所选动作复制 / 移动到根目录对应类型子目录
                </Text>
              }
            >
              <Input
                placeholder="/abs/path/to/source"
                allowClear
                addonAfter={
                  <AntSpace size={4}>
                    <Button
                      size="small"
                      type="text"
                      icon={<FileOutlined />}
                      onClick={() => handlePick("file")}
                    >
                      选文件
                    </Button>
                    <Button
                      size="small"
                      type="text"
                      icon={<FolderOpenOutlined />}
                      onClick={() => handlePick("directory")}
                    >
                      选目录
                    </Button>
                  </AntSpace>
                }
              />
            </Form.Item>

            <ReferenceBaseFields />
          </Form>
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
                    <Text style={{ fontFamily: "monospace", fontSize: 12 }}>{plan.source}</Text>
                  ),
                },
                {
                  key: "target",
                  label: "目标路径",
                  children: (
                    <Text style={{ fontFamily: "monospace", fontSize: 12 }}>{targetPreview}</Text>
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
                  activeConflicts.length > 0
                    ? "目标名仍与既有项冲突，请更换"
                    : undefined
                }
              >
                <Input
                  value={targetName}
                  onChange={(e) => setTargetName(e.target.value)}
                  placeholder="目标名（缺省用源名）"
                  allowClear
                />
              </Form.Item>

              <Form.Item label="托管动作">
                <Radio.Group
                  value={managedAction}
                  onChange={(e) => setManagedAction(e.target.value as ManagedAction)}
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

            {managedAction === "move" && (
              <Alert
                type="info"
                showIcon
                message="移动动作将删除源文件"
                description="确认后源路径下的文件将被移动到目标位置，此操作不可撤销。"
              />
            )}

            {/* m3-3.7 进度展示：confirmed 进行中渲染 Progress；多任务通过 refId 区分（本对话框同一时刻仅一个任务） */}
            {confirmLoading && currentRefId && (
              <div style={{ marginTop: 16 }}>
                <Text type="secondary" style={{ fontSize: 12 }}>
                  正在{managedAction === "move" ? "移动" : "复制"}文件…
                </Text>
                {(() => {
                  // total === 0（空文件/空目录）显示 indeterminate 态
                  const total = currentProgress?.total ?? 0;
                  const bytes = currentProgress?.bytes ?? 0;
                  const percent =
                    total > 0 ? Math.min(100, Math.round((bytes / total) * 100)) : 0;
                  const status = progressFailed ? ("exception" as const) : undefined;
                  if (total === 0) {
                    return (
                      <Progress
                        percent={100}
                        status={progressFailed ? "exception" : "active"}
                        showInfo={false}
                      />
                    );
                  }
                  return (
                    <>
                      <Progress percent={percent} status={status} />
                      <Text type="secondary" style={{ fontSize: 12 }}>
                        {formatBytes(bytes)} / {formatBytes(total)}
                      </Text>
                    </>
                  );
                })()}
              </div>
            )}

            {/* 失败短暂停留期间也保留进度条显示（status=exception） */}
            {!confirmLoading && progressFailed && currentRefId && currentProgress && (
              <div style={{ marginTop: 16 }}>
                <Progress
                  percent={
                    currentProgress.total > 0
                      ? Math.min(
                          100,
                          Math.round((currentProgress.bytes / currentProgress.total) * 100),
                        )
                      : 100
                  }
                  status="exception"
                />
                <Text type="danger" style={{ fontSize: 12 }}>
                  导入失败，对话框即将关闭…
                </Text>
              </div>
            )}
          </>
        ) : null}
      </Modal>
    </>
  );
}
