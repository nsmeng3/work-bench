import { useEffect, useState } from "react";
import {
  Alert,
  Descriptions,
  Modal,
  Space as AntSpace,
  Spin,
  Tag,
  Typography,
  message,
} from "antd";
import type { Reference, UndoPlan } from "../api";
import { refUndoImport, toApiError } from "../api";

const { Text, Paragraph } = Typography;

/**
 * 撤销导入确认对话框 — m4-4.9 · 任务包 tasks/m4-4.9.md。
 *
 * 流程：
 * 1. 打开时调 `ref_undo_import(refId, confirmed=false)` 拿 UndoPlan。
 * 2. 渲染 UndoPlan 信息（refName / managedAction / currentPath / originalSource）。
 * 3. blockers 非空时确认按钮禁用，并列出所有阻塞原因。
 * 4. 用户确认后调 `ref_undo_import(refId, confirmed=true)` 执行撤销。
 * 5. 成功 toast「已撤销导入」+ 回调 onSuccess 刷新列表。
 *
 * 契约要点：
 * - 仅 managed 导入可撤销（hosting != managed 时 blockers 非空）。
 * - 24h 撤销窗口（后端 UNDO_WINDOW_SECS = 24 * 3600）。
 * - copy 撤销：删除目标文件 + 删除引用。
 * - move 撤销：把目标文件移回 originalSource + 删除引用；源被占 → FS_TARGET_EXISTS 中止。
 */

interface UndoImportDialogProps {
  /** 目标引用；null 表示对话框关闭 */
  reference: Reference | null;
  /** 关闭对话框（无论成功或取消） */
  onClose: () => void;
  /** 操作成功后回调（父组件通常用来刷新引用列表） */
  onSuccess: () => void;
}

export function UndoImportDialog({ reference, onClose, onSuccess }: UndoImportDialogProps) {
  const [plan, setPlan] = useState<UndoPlan | null>(null);
  const [planLoading, setPlanLoading] = useState(false);
  const [planError, setPlanError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [messageApi, messageContextHolder] = message.useMessage();

  const open = reference !== null;

  // 打开对话框时立即调 plan
  useEffect(() => {
    if (!open || !reference) return;
    let cancelled = false;
    setPlan(null);
    setPlanError(null);
    setPlanLoading(true);
    refUndoImport(reference.id, false)
      .then((p) => {
        if (!cancelled) setPlan(p);
      })
      .catch((err) => {
        if (!cancelled) setPlanError(toApiError(err).message);
      })
      .finally(() => {
        if (!cancelled) setPlanLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [open, reference]);

  if (!open || !reference) {
    return <>{messageContextHolder}</>;
  }

  const handleCancel = () => {
    if (submitting) return;
    onClose();
  };

  const handleConfirm = async () => {
    setSubmitting(true);
    try {
      await refUndoImport(reference.id, true);
      messageApi.success({ content: "已撤销导入", duration: 2 });
      onSuccess();
      onClose();
    } catch (err) {
      const apiErr = toApiError(err);
      messageApi.error({
        content: apiErr.retryable ? `${apiErr.message}（可重试）` : apiErr.message,
        duration: 3,
      });
    } finally {
      setSubmitting(false);
    }
  };

  const canConfirm = plan !== null && plan.canUndo && !planLoading && !planError;

  return (
    <>
      {messageContextHolder}
      <Modal
        title="撤销导入"
        open
        onOk={handleConfirm}
        onCancel={handleCancel}
        confirmLoading={submitting}
        okText="确认撤销"
        okButtonProps={{ danger: true, disabled: !canConfirm }}
        cancelText="取消"
      >
        <AntSpace direction="vertical" size={12} style={{ width: "100%" }}>
          <Alert
            type="warning"
            showIcon
            message="撤销导入将删除托管文件并移除引用"
            description="copy 导入：删除目标文件；move 导入：把文件移回原始位置。此操作不可恢复。"
          />
          {planLoading && (
            <div style={{ textAlign: "center", padding: "16px 0" }}>
              <Spin /> <Text type="secondary">正在检查撤销条件...</Text>
            </div>
          )}
          {planError && (
            <Alert type="error" showIcon message="撤销计划加载失败" description={planError} />
          )}
          {plan && (
            <>
              <Descriptions
                bordered
                size="small"
                column={1}
                items={[
                  { key: "refName", label: "引用名称", children: plan.refName },
                  {
                    key: "managedAction",
                    label: "导入方式",
                    children: (
                      <Tag color={plan.managedAction === "copy" ? "blue" : "orange"}>
                        {plan.managedAction === "copy" ? "复制（copy）" : "移动（move）"}
                      </Tag>
                    ),
                  },
                  {
                    key: "currentPath",
                    label: "当前位置",
                    children: <Text code style={{ fontSize: 12 }}>{plan.currentPath}</Text>,
                  },
                  {
                    key: "originalSource",
                    label: "原始位置",
                    children: plan.originalSource ? (
                      <Text code style={{ fontSize: 12 }}>{plan.originalSource}</Text>
                    ) : (
                      <Text type="secondary">未记录</Text>
                    ),
                  },
                ]}
              />
              {plan.blockers.length > 0 && (
                <Alert
                  type="error"
                  showIcon
                  message="无法撤销"
                  description={
                    <ul style={{ margin: 0, paddingLeft: 20 }}>
                      {plan.blockers.map((b, i) => (
                        <li key={i}>{b}</li>
                      ))}
                    </ul>
                  }
                />
              )}
              {plan.canUndo && (
                <Paragraph type="secondary" style={{ margin: 0 }}>
                  {plan.managedAction === "copy"
                    ? "确认后将删除当前位置的文件，并移除该引用。"
                    : "确认后将把文件移回原始位置，并移除该引用。"}
                </Paragraph>
              )}
            </>
          )}
        </AntSpace>
      </Modal>
    </>
  );
}
