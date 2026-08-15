import { useEffect, useState } from "react";
import {
  Alert,
  Descriptions,
  Input,
  Modal,
  Space as AntSpace,
  Spin,
  Typography,
  message,
} from "antd";
import type { DispPreview, Reference } from "../api";
import {
  dispArchive,
  dispDestroy,
  dispPreview,
  dispSoftDelete,
  dispUnarchive,
  toApiError,
} from "../api";
import type { DispositionAction } from "./DispositionButtons";

const { Text, Paragraph } = Typography;

/**
 * 三档处置确认对话框 — 详细设计 §2.6 / §4.2。
 *
 * - **归档 / 恢复**：轻量确认（无 preview），文案「归档后不再出现在主列表，可随时恢复」。
 * - **删除**：`Modal` + `disp_preview` 数据（文件数 / 大小 / 目标路径），文案「文件将移入系统回收站」。
 * - **销毁**：`Modal` + `disp_preview` 数据 + 输入框要求输入资源完整名称；
 *   不一致时确认按钮禁用；文案「**此操作不可恢复**，请输入完整名称确认」。
 *
 * 错误处理：
 * - `COMMON_CONFIRM_REQUIRED` → 提示「名称不匹配」
 * - `FS_RECYCLE_UNSUPPORTED` → 「当前系统不支持回收站，只能归档或销毁」
 * - 其他按统一错误模型 message
 */

interface DispositionConfirmDialogProps {
  /** 当前要执行的动作；null 表示对话框关闭 */
  action: DispositionAction | null;
  /** 目标引用 */
  reference: Reference | null;
  /** 关闭对话框（无论成功或取消） */
  onClose: () => void;
  /** 操作成功后回调（父组件通常用来刷新引用列表） */
  onSuccess: () => void;
}

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

export function DispositionConfirmDialog({
  action,
  reference,
  onClose,
  onSuccess,
}: DispositionConfirmDialogProps) {
  const [preview, setPreview] = useState<DispPreview | null>(null);
  const [previewLoading, setPreviewLoading] = useState(false);
  const [previewError, setPreviewError] = useState<string | null>(null);
  const [confirmText, setConfirmText] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [messageApi, messageContextHolder] = message.useMessage();

  const open = action !== null && reference !== null;
  const needPreview = action === "softDelete" || action === "destroy";

  // 打开对话框时立即调 disp_preview（仅删除/销毁需要）
  useEffect(() => {
    if (!open || !needPreview || !reference) return;
    let cancelled = false;
    setPreview(null);
    setPreviewError(null);
    setPreviewLoading(true);
    dispPreview(reference.id)
      .then((p) => {
        if (!cancelled) setPreview(p);
      })
      .catch((err) => {
        if (!cancelled) setPreviewError(toApiError(err).message);
      })
      .finally(() => {
        if (!cancelled) setPreviewLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [open, needPreview, reference]);

  // 切换 action / reference 时清空 confirmText
  useEffect(() => {
    setConfirmText("");
  }, [action, reference?.id]);

  if (!open || !reference || !action) {
    return <>{messageContextHolder}</>;
  }

  const handleCancel = () => {
    if (submitting) return;
    onClose();
  };

  const showError = (err: unknown) => {
    const apiErr = toApiError(err);
    let content: string;
    if (apiErr.code === "COMMON_CONFIRM_REQUIRED") {
      content = "名称不匹配";
    } else if (apiErr.code === "FS_RECYCLE_UNSUPPORTED") {
      content = "当前系统不支持回收站，只能归档或销毁";
    } else {
      content = apiErr.retryable ? `${apiErr.message}（可重试）` : apiErr.message;
    }
    messageApi.error({ content, duration: 3 });
  };

  const handleArchive = async () => {
    setSubmitting(true);
    try {
      await dispArchive(reference.id);
      messageApi.success({ content: "已归档", duration: 2 });
      onSuccess();
      onClose();
    } catch (err) {
      showError(err);
    } finally {
      setSubmitting(false);
    }
  };

  const handleUnarchive = async () => {
    setSubmitting(true);
    try {
      await dispUnarchive(reference.id);
      messageApi.success({ content: "已恢复", duration: 2 });
      onSuccess();
      onClose();
    } catch (err) {
      showError(err);
    } finally {
      setSubmitting(false);
    }
  };

  const handleSoftDelete = async () => {
    setSubmitting(true);
    try {
      await dispSoftDelete(reference.id);
      messageApi.success({ content: "已移入回收站", duration: 2 });
      onSuccess();
      onClose();
    } catch (err) {
      showError(err);
    } finally {
      setSubmitting(false);
    }
  };

  const handleDestroy = async () => {
    setSubmitting(true);
    try {
      await dispDestroy(reference.id, confirmText);
      messageApi.success({ content: "已永久销毁", duration: 2 });
      onSuccess();
      onClose();
    } catch (err) {
      showError(err);
    } finally {
      setSubmitting(false);
    }
  };

  /* ---------------- 归档（轻量） ---------------- */
  if (action === "archive") {
    return (
      <>
        {messageContextHolder}
        <Modal
          title="归档引用"
          open
          onOk={handleArchive}
          onCancel={handleCancel}
          confirmLoading={submitting}
          okText="归档"
          cancelText="取消"
        >
          <Paragraph>
            归档后不再出现在主列表，可随时恢复。
          </Paragraph>
          <Text type="secondary">目标引用：{reference.name}</Text>
        </Modal>
      </>
    );
  }

  /* ---------------- 恢复（轻量） ---------------- */
  if (action === "unarchive") {
    return (
      <>
        {messageContextHolder}
        <Modal
          title="恢复引用"
          open
          onOk={handleUnarchive}
          onCancel={handleCancel}
          confirmLoading={submitting}
          okText="恢复"
          cancelText="取消"
        >
          <Paragraph>恢复后引用重新出现在主列表。</Paragraph>
          <Text type="secondary">目标引用：{reference.name}</Text>
        </Modal>
      </>
    );
  }

  /* ---------------- 删除（回收站） ---------------- */
  if (action === "softDelete") {
    return (
      <>
        {messageContextHolder}
        <Modal
          title="删除引用（移入回收站）"
          open
          onOk={handleSoftDelete}
          onCancel={handleCancel}
          confirmLoading={submitting}
          okText="移入回收站"
          okButtonProps={{ danger: true, disabled: previewLoading || !!previewError }}
          cancelText="取消"
        >
          <AntSpace direction="vertical" size={12} style={{ width: "100%" }}>
            <Alert
              type="warning"
              showIcon
              message="文件将移入系统回收站"
              description="可以从系统回收站手动恢复；引用状态将标记为已删除。"
            />
            {previewLoading && (
              <div style={{ textAlign: "center", padding: "16px 0" }}>
                <Spin /> <Text type="secondary">正在统计目标...</Text>
              </div>
            )}
            {previewError && (
              <Alert type="error" showIcon message="预览加载失败" description={previewError} />
            )}
            {preview && (
              <Descriptions
                bordered
                size="small"
                column={1}
                items={[
                  { key: "target", label: "目标路径", children: <Text code>{preview.target}</Text> },
                  {
                    key: "kind",
                    label: "类型",
                    children: preview.isDir ? "目录" : "文件",
                  },
                  {
                    key: "fileCount",
                    label: "文件数",
                    children: preview.fileCount,
                  },
                  {
                    key: "totalBytes",
                    label: "总大小",
                    children: formatBytes(preview.totalBytes),
                  },
                ]}
              />
            )}
            <Text type="secondary">目标引用：{reference.name}</Text>
          </AntSpace>
        </Modal>
      </>
    );
  }

  /* ---------------- 销毁（永久删除） ---------------- */
  const confirmMatched = confirmText === reference.name;
  return (
    <>
      {messageContextHolder}
      <Modal
        title="销毁引用（永久删除）"
        open
        onOk={handleDestroy}
        onCancel={handleCancel}
        confirmLoading={submitting}
        okText="永久销毁"
        okButtonProps={{
          danger: true,
          disabled: !confirmMatched || previewLoading || !!previewError,
        }}
        cancelText="取消"
        maskClosable={false}
      >
        <AntSpace direction="vertical" size={12} style={{ width: "100%" }}>
          <Alert
            type="error"
            showIcon
            message="此操作不可恢复"
            description="文件将被永久删除（不经回收站），引用行也将从数据库物理删除。请输入完整名称确认。"
          />
          {previewLoading && (
            <div style={{ textAlign: "center", padding: "16px 0" }}>
              <Spin /> <Text type="secondary">正在统计目标...</Text>
            </div>
          )}
          {previewError && (
            <Alert type="error" showIcon message="预览加载失败" description={previewError} />
          )}
          {preview && (
            <>
              <Descriptions
                bordered
                size="small"
                column={1}
                items={[
                  { key: "target", label: "目标路径", children: <Text code>{preview.target}</Text> },
                  {
                    key: "kind",
                    label: "类型",
                    children: preview.isDir ? "目录" : "文件",
                  },
                  {
                    key: "fileCount",
                    label: "文件数",
                    children: preview.fileCount,
                  },
                  {
                    key: "totalBytes",
                    label: "总大小",
                    children: formatBytes(preview.totalBytes),
                  },
                ]}
              />
              <Alert type="warning" showIcon message={preview.warning} />
            </>
          )}
          <div>
            <Text strong>请输入资源完整名称以确认销毁：</Text>
            <Paragraph type="secondary" style={{ margin: "4px 0 8px" }}>
              <Text code>{reference.name}</Text>
            </Paragraph>
            <Input
              value={confirmText}
              onChange={(e) => setConfirmText(e.target.value)}
              placeholder={reference.name}
              status={confirmText.length > 0 && !confirmMatched ? "error" : undefined}
              autoFocus
            />
            {confirmText.length > 0 && !confirmMatched && (
              <Text type="danger" style={{ fontSize: 12 }}>
                名称不匹配
              </Text>
            )}
          </div>
        </AntSpace>
      </Modal>
    </>
  );
}
