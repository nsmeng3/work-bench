import { useEffect, useMemo, useState } from "react";
import {
  Alert,
  Button,
  Form,
  List,
  Modal,
  Radio,
  Space,
  Steps,
  Tag,
  Typography,
  message,
} from "antd";
import {
  CheckCircleFilled,
  CloseCircleFilled,
  WarningFilled,
} from "@ant-design/icons";
import type {
  ChangeRootStrategy,
  MigrationFailure,
  MigrationPlan,
  MigrationPlanItem,
  MigrationResult,
} from "../api";
import {
  isMigrationPlan,
  isMigrationResult,
  settingsChangeRootDir,
  toApiError,
} from "../api";
import { RootDirPicker } from "./RootDirPicker";

interface MigrationWizardProps {
  /** 弹窗开关 */
  open: boolean;
  /** 关闭回调（任何阶段触发均视为取消/结束） */
  onClose: () => void;
  /** 迁移成功回调（携带新根目录） */
  onSuccess: (newRootDir: string) => void;
}

type WizardStep = "input" | "confirm" | "result";

/** 字节数格式化（B/KB/MB/GB） */
function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return "-";
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let value = bytes;
  let idx = -1;
  do {
    value /= 1024;
    idx += 1;
  } while (value >= 1024 && idx < units.length - 1);
  return `${value.toFixed(value >= 100 ? 0 : value >= 10 ? 1 : 2)} ${units[idx]}`;
}

/** 状态标签颜色与文案 */
function statusTag(status: MigrationPlanItem["status"]) {
  switch (status) {
    case "ok":
      return <Tag color="green">可迁移</Tag>;
    case "source_missing":
      return <Tag color="orange">源缺失</Tag>;
    case "target_conflict":
      return <Tag color="red">目标冲突</Tag>;
    default:
      return <Tag>{status}</Tag>;
  }
}

/**
 * 根目录迁移确认向导 — M6-6.6。
 *
 * 三阶段流程（§4.4）：
 * 1. input：选择新根目录 + 策略（future_only / migrate）
 * 2. confirm：调用 settings_change_root_dir(confirmed=false) 拿 MigrationPlan，
 *    展示影响列表（含冲突/源缺失高亮），用户确认后调 confirmed=true
 * 3. result：展示 MigrationResult（成功列表 + 失败列表含错误原因）
 *
 * 策略说明：
 * - future_only：仅改 settings.root_dir，不动已有文件；confirmed=true 时直接成功
 * - migrate：逐项复制 + 更新 locator_json；单项失败记入 failed 不中断
 */
export function MigrationWizard({ open, onClose, onSuccess }: MigrationWizardProps) {
  const [messageApi, contextHolder] = message.useMessage();
  const [step, setStep] = useState<WizardStep>("input");
  const [form] = Form.useForm<{ newRootDir: string; strategy: ChangeRootStrategy }>();

  const [submitting, setSubmitting] = useState(false);
  const [plan, setPlan] = useState<MigrationPlan | null>(null);
  const [result, setResult] = useState<MigrationResult | null>(null);
  const [appliedFutureOnly, setAppliedFutureOnly] = useState<string | null>(null);

  // 弹窗每次打开时重置状态
  useEffect(() => {
    if (open) {
      setStep("input");
      setPlan(null);
      setResult(null);
      setAppliedFutureOnly(null);
      form.setFieldsValue({ newRootDir: "", strategy: "future_only" });
    }
  }, [open, form]);

  const stepIndex = useMemo(() => {
    switch (step) {
      case "input":
        return 0;
      case "confirm":
        return 1;
      case "result":
        return 2;
    }
  }, [step]);

  /** 第一步：下一步 — 调 plan 阶段或直接 future_only apply */
  async function handleNext() {
    let values: { newRootDir: string; strategy: ChangeRootStrategy };
    try {
      values = await form.validateFields();
    } catch {
      return;
    }
    const newRootDir = values.newRootDir.trim();
    const strategy = values.strategy;

    setSubmitting(true);
    try {
      if (strategy === "future_only") {
        // future_only：直接 confirmed=true 应用（无影响列表可预览）
        const r = await settingsChangeRootDir({
          newRootDir,
          strategy,
          confirmed: true,
        });
        if (isMigrationResult(r) || isMigrationPlan(r)) {
          // 后端契约异常：future_only 不应返回 plan/result
          messageApi.warning("后端返回了非预期结果，请检查日志");
          return;
        }
        setAppliedFutureOnly(r.newRootDir);
        setResult(null);
        setStep("result");
      } else {
        // migrate：先 confirmed=false 拿 MigrationPlan
        const r = await settingsChangeRootDir({
          newRootDir,
          strategy,
          confirmed: false,
        });
        if (!isMigrationPlan(r)) {
          messageApi.warning("后端返回了非预期结果，请检查日志");
          return;
        }
        setPlan(r);
        setStep("confirm");
      }
    } catch (err) {
      const apiErr = toApiError(err);
      messageApi.error(
        apiErr.retryable ? `${apiErr.message}（可重试）` : apiErr.message,
      );
    } finally {
      setSubmitting(false);
    }
  }

  /** 第二步：确认迁移 — 调 confirmed=true 执行 */
  async function handleConfirmMigrate() {
    if (!plan) return;
    setSubmitting(true);
    try {
      const r = await settingsChangeRootDir({
        newRootDir: plan.newRootDir,
        strategy: plan.strategy,
        confirmed: true,
      });
      if (!isMigrationResult(r)) {
        messageApi.warning("后端返回了非预期结果，请检查日志");
        return;
      }
      setResult(r);
      setStep("result");
    } catch (err) {
      const apiErr = toApiError(err);
      messageApi.error(
        apiErr.retryable ? `${apiErr.message}（可重试）` : apiErr.message,
      );
    } finally {
      setSubmitting(false);
    }
  }

  /** 第三步：完成 — 关闭并回调 onSuccess */
  function handleFinish() {
    const finalRoot = appliedFutureOnly ?? plan?.newRootDir;
    if (finalRoot) {
      onSuccess(finalRoot);
    }
    onClose();
  }

  /** 第二步：返回第一步 */
  function handleBackToInput() {
    setPlan(null);
    setStep("input");
  }

  const migratableCount = plan?.items.filter((i) => i.status === "ok").length ?? 0;
  const conflictCount = plan?.items.filter((i) => i.status === "target_conflict").length ?? 0;
  const missingCount = plan?.items.filter((i) => i.status === "source_missing").length ?? 0;

  return (
    <Modal
      title="修改资源根目录"
      open={open}
      onCancel={onClose}
      footer={null}
      width={720}
      destroyOnHidden
      maskClosable={false}
    >
      {contextHolder}
      <Steps
        current={stepIndex}
        size="small"
        style={{ marginBottom: 24 }}
        items={[
          { title: "选择新根目录" },
          { title: "确认迁移计划" },
          { title: "迁移结果" },
        ]}
      />

      {/* 第一步：输入新根目录 + 策略 */}
      {step === "input" && (
        <Form form={form} layout="vertical" initialValues={{ strategy: "future_only" }}>
          <Form.Item
            name="newRootDir"
            label="新根目录路径"
            rules={[
              { required: true, message: "请输入新根目录路径" },
              {
                pattern: /^\/.*/,
                message: "请输入绝对路径（以 / 开头）",
              },
            ]}
          >
            <RootDirPicker placeholder="例如 /Users/you/NewWorkbench" />
          </Form.Item>

          <Form.Item
            name="strategy"
            label="迁移策略"
            rules={[{ required: true }]}
          >
            <Radio.Group>
              <Space direction="vertical">
                <Radio value="future_only">
                  仅改未来默认
                  <Typography.Text type="secondary" style={{ marginLeft: 8, fontSize: 12 }}>
                    只更新设置，已有文件与引用保持原路径
                  </Typography.Text>
                </Radio>
                <Radio value="migrate">
                  迁移已有内容
                  <Typography.Text type="secondary" style={{ marginLeft: 8, fontSize: 12 }}>
                    逐项复制已托管文件到新根目录，并更新引用记录
                  </Typography.Text>
                </Radio>
              </Space>
            </Radio.Group>
          </Form.Item>

          <div style={{ textAlign: "right" }}>
            <Space>
              <Button onClick={onClose}>取消</Button>
              <Button type="primary" onClick={handleNext} loading={submitting}>
                下一步
              </Button>
            </Space>
          </div>
        </Form>
      )}

      {/* 第二步：确认迁移计划 */}
      {step === "confirm" && plan && (
        <div>
          <Alert
            type="info"
            showIcon
            style={{ marginBottom: 16 }}
            message={
              <Space size={16} wrap>
                <span>
                  新根目录：<Typography.Text code>{plan.newRootDir}</Typography.Text>
                </span>
                <span>
                  总大小：<Typography.Text strong>{formatBytes(plan.totalBytes)}</Typography.Text>
                </span>
                <span>
                  可迁移 <Typography.Text strong style={{ color: "#52c41a" }}>{migratableCount}</Typography.Text> 项
                </span>
                {conflictCount > 0 && (
                  <span>
                    冲突 <Typography.Text strong style={{ color: "#ff4d4f" }}>{conflictCount}</Typography.Text> 项
                  </span>
                )}
                {missingCount > 0 && (
                  <span>
                    源缺失 <Typography.Text strong style={{ color: "#faad14" }}>{missingCount}</Typography.Text> 项
                  </span>
                )}
              </Space>
            }
          />

          {conflictCount > 0 && (
            <Alert
              type="warning"
              showIcon
              style={{ marginBottom: 16 }}
              message="存在目标冲突项"
              description="冲突项在确认迁移时将被跳过并记入失败列表。如需保留，请取消并手动清理目标目录。"
            />
          )}

          {plan.items.length === 0 ? (
            <Alert
              type="info"
              showIcon
              message="当前没有已托管的引用需要迁移"
              style={{ marginBottom: 16 }}
            />
          ) : (
            <List<MigrationPlanItem>
              size="small"
              bordered
              dataSource={plan.items}
              rowKey="refId"
              style={{ maxHeight: 320, overflow: "auto", marginBottom: 16 }}
              renderItem={(item) => (
                <List.Item>
                  <List.Item.Meta
                    title={
                      <Space size={8}>
                        {statusTag(item.status)}
                        <Typography.Text code style={{ fontSize: 12 }}>
                          {item.currentPath}
                        </Typography.Text>
                        <Typography.Text type="secondary">→</Typography.Text>
                        <Typography.Text code style={{ fontSize: 12 }}>
                          {item.proposedPath}
                        </Typography.Text>
                      </Space>
                    }
                  />
                </List.Item>
              )}
            />
          )}

          <div style={{ textAlign: "right" }}>
            <Space>
              <Button onClick={handleBackToInput} disabled={submitting}>
                返回
              </Button>
              <Button onClick={onClose} disabled={submitting}>
                取消
              </Button>
              <Button
                type="primary"
                onClick={handleConfirmMigrate}
                loading={submitting}
                disabled={migratableCount === 0 && plan.items.length > 0}
              >
                确认迁移
              </Button>
            </Space>
          </div>
        </div>
      )}

      {/* 第三步：迁移结果 */}
      {step === "result" && (
        <div>
          {appliedFutureOnly && (
            <Alert
              type="success"
              showIcon
              icon={<CheckCircleFilled />}
              style={{ marginBottom: 16 }}
              message="根目录已更新"
              description={
                <>
                  新根目录：<Typography.Text code>{appliedFutureOnly}</Typography.Text>
                  <br />
                  <Typography.Text type="secondary" style={{ fontSize: 12 }}>
                    已有文件与引用保持原路径，仅未来新建的托管内容会使用新根目录。
                  </Typography.Text>
                </>
              }
            />
          )}

          {result && (
            <>
              <Alert
                type={result.failed.length === 0 ? "success" : "warning"}
                showIcon
                icon={result.failed.length === 0 ? <CheckCircleFilled /> : <WarningFilled />}
                style={{ marginBottom: 16 }}
                message={
                  result.failed.length === 0
                    ? `迁移完成：成功 ${result.migrated.length} 项`
                    : `迁移部分完成：成功 ${result.migrated.length} 项，失败 ${result.failed.length} 项`
                }
              />

              {result.migrated.length > 0 && (
                <>
                  <Typography.Title level={5} style={{ marginTop: 0 }}>
                    <CheckCircleFilled style={{ color: "#52c41a", marginRight: 8 }} />
                    成功（{result.migrated.length}）
                  </Typography.Title>
                  <List
                    size="small"
                    bordered
                    dataSource={result.migrated}
                    rowKey={(id) => id}
                    style={{ maxHeight: 160, overflow: "auto", marginBottom: 16 }}
                    renderItem={(refId) => (
                      <List.Item>
                        <Typography.Text code style={{ fontSize: 12 }}>
                          {refId}
                        </Typography.Text>
                      </List.Item>
                    )}
                  />
                </>
              )}

              {result.failed.length > 0 && (
                <>
                  <Typography.Title level={5}>
                    <CloseCircleFilled style={{ color: "#ff4d4f", marginRight: 8 }} />
                    失败（{result.failed.length}）
                  </Typography.Title>
                  <List<MigrationFailure>
                    size="small"
                    bordered
                    dataSource={result.failed}
                    rowKey="refId"
                    style={{ maxHeight: 200, overflow: "auto", marginBottom: 16 }}
                    renderItem={(f) => (
                      <List.Item>
                        <List.Item.Meta
                          title={
                            <Typography.Text code style={{ fontSize: 12 }}>
                              {f.currentPath} → {f.proposedPath}
                            </Typography.Text>
                          }
                          description={
                            <Typography.Text type="danger" style={{ fontSize: 12 }}>
                              {f.error}
                            </Typography.Text>
                          }
                        />
                      </List.Item>
                    )}
                  />
                </>
              )}
            </>
          )}

          <div style={{ textAlign: "right" }}>
            <Button type="primary" onClick={handleFinish}>
              完成
            </Button>
          </div>
        </div>
      )}
    </Modal>
  );
}
