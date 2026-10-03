import { useState } from "react";
import {
  Alert,
  Button,
  DatePicker,
  Descriptions,
  Dropdown,
  Empty,
  Form,
  Input,
  Modal,
  Space as AntSpace,
  Spin,
  Typography,
  message,
} from "antd";
import {
  CheckOutlined,
  ClockCircleOutlined,
  CopyOutlined,
  DeleteOutlined,
  DownOutlined,
  FolderOpenOutlined,
  PlayCircleOutlined,
} from "@ant-design/icons";
import type { MenuProps } from "antd";
import type {
  InboxIgnoreRuleKind,
  InboxItem,
  InboxItemDetail,
  ReferenceType,
} from "../api";
import { inboxIgnore, inboxSnooze, toApiError } from "../api";
import { formatRelativeTime } from "./InboxItemCard";

/**
 * 收件箱详情面板 — 任务包 m5-5.6。
 *
 * 显示：完整路径 / 建议类型 / 文件大小 / 发现时间 / 预览（文本前 N 行）。
 * 敏感文件（sensitiveWarning 非空）：黄色 Alert + 不显示预览（§6.9）。
 *
 * 操作按钮：
 * - 「处理」（主要按钮）：跳 5.7 处理对话框（由父组件 onProcess 回调）
 * - 「暂后」：Modal 输入 note/remindAt，调 inbox_snooze
 * - 「忽略」：Dropdown 选 once/by_ext/by_name/by_dir，调 inbox_ignore
 */

const TYPE_LABELS: Record<ReferenceType, string> = {
  code: "代码",
  document: "文档",
  data: "数据",
  artifact: "制品",
  tool: "工具",
  media: "媒体",
};

function humanSize(bytes?: number | null): string {
  if (bytes === null || bytes === undefined || !Number.isFinite(bytes)) return "-";
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let v = bytes;
  let u = -1;
  do {
    v /= 1024;
    u += 1;
  } while (v >= 1024 && u < units.length - 1);
  return `${v.toFixed(2)} ${units[u]}`;
}

function formatUnixSeconds(ts?: number | null): string {
  if (ts === null || ts === undefined || !Number.isFinite(ts)) return "-";
  return new Date(ts * 1000).toLocaleString();
}

/** 从路径推导忽略规则的默认值 */
function deriveIgnoreValue(
  item: InboxItem,
  kind: Exclude<InboxIgnoreRuleKind, "once">,
): string {
  const name = item.path.split("/").filter(Boolean).pop() ?? item.path;
  if (kind === "by_ext") return item.ext ? `.${item.ext}` : "";
  if (kind === "by_name") return name;
  // by_dir：去掉文件名
  const idx = item.path.lastIndexOf("/");
  return idx > 0 ? item.path.slice(0, idx) : item.path;
}

const IGNORE_MENU_LABELS: Record<InboxIgnoreRuleKind, string> = {
  once: "仅此次",
  by_ext: "按扩展名忽略",
  by_name: "按文件名忽略",
  by_dir: "按目录忽略",
};

interface InboxDetailPanelProps {
  /** 当前选中条目的详情；null 表示未选中 */
  detail: InboxItemDetail | null;
  loading: boolean;
  /** 点击「处理」按钮（跳 5.7 处理对话框） */
  onProcess: (item: InboxItem) => void;
  /** snooze/ignore 成功后通知父组件刷新列表与详情 */
  onChanged: (updated: InboxItem) => void;
}

export function InboxDetailPanel({
  detail,
  loading,
  onProcess,
  onChanged,
}: InboxDetailPanelProps) {
  const [snoozeOpen, setSnoozeOpen] = useState(false);
  /** DatePicker 的 value 类型由 antd 内部 dayjs 决定；这里用 unknown 接，调 .unix() 取值 */
  const [snoozeForm] = Form.useForm<{ note?: string; remindAt?: { unix(): number } }>();
  const [submitting, setSubmitting] = useState(false);

  if (loading) {
    return (
      <div style={{ padding: 48, textAlign: "center" }}>
        <Spin tip="加载详情…" />
      </div>
    );
  }

  if (!detail) {
    return (
      <div style={{ padding: 48 }}>
        <Empty description="从左侧列表选择一条收件箱条目查看详情" />
      </div>
    );
  }

  const isPending = detail.status === "pending";
  const isSnoozed = detail.status === "snoozed";
  const canOperate = isPending || isSnoozed;

  async function handleSnoozeSubmit() {
    if (!detail) return;
    const values = await snoozeForm.validateFields();
    setSubmitting(true);
    try {
      const updated = await inboxSnooze({
        id: detail.id,
        note: values.note?.trim() || undefined,
        remindAt: values.remindAt ? values.remindAt.unix() : undefined,
      });
      message.success("已暂后");
      setSnoozeOpen(false);
      snoozeForm.resetFields();
      onChanged(updated);
    } catch (err) {
      const apiErr = toApiError(err);
      message.error(`暂后失败：${apiErr.message}`);
    } finally {
      setSubmitting(false);
    }
  }

  async function handleIgnore(kind: InboxIgnoreRuleKind) {
    if (!detail) return;
    setSubmitting(true);
    try {
      const rule =
        kind === "once"
          ? { kind: "once" as const }
          : { kind, value: deriveIgnoreValue(detail, kind) };
      const updated = await inboxIgnore({ id: detail.id, rule });
      message.success(
        kind === "once" ? "已忽略" : `已忽略并创建规则（${IGNORE_MENU_LABELS[kind]}）`,
      );
      onChanged(updated);
    } catch (err) {
      const apiErr = toApiError(err);
      message.error(`忽略失败：${apiErr.message}`);
    } finally {
      setSubmitting(false);
    }
  }

  /* ---------------- M7-1 · 收件箱条目直接按 path 操作 ----------------
   *
   * 注意：inbox 条目尚未转为 resource_reference，没有 ref_id，
   * 因此不能调 ref_open / ref_reveal_in_finder（它们按 ref_id 查 locator）。
   * 这里直接用 @tauri-apps/plugin-opener 的 openPath / revealItemInDir
   * 按 detail.path 操作系统文件。
   *
   * 不写 ref_access_log：外键约束要求 ref_id 存在于 resource_reference，
   * inbox 条目不在该表。M7-3 Dashboard 的"最近资源"仅统计正式引用。
   */

  async function handleOpenPath() {
    if (!detail) return;
    try {
      const opener = await import("@tauri-apps/plugin-opener");
      await opener.openPath(detail.path);
    } catch (err) {
      console.warn("openPath 失败：", err);
      message.error(`打开失败：${err instanceof Error ? err.message : String(err)}`);
    }
  }

  async function handleRevealPath() {
    if (!detail) return;
    try {
      const opener = await import("@tauri-apps/plugin-opener");
      await opener.revealItemInDir(detail.path);
    } catch (err) {
      console.warn("revealItemInDir 失败：", err);
      message.error(`定位失败：${err instanceof Error ? err.message : String(err)}`);
    }
  }

  async function handleCopyPath() {
    if (!detail) return;
    try {
      await navigator.clipboard.writeText(detail.path);
      message.success("路径已复制");
    } catch (err) {
      console.warn("复制路径失败：", err);
      message.error("复制失败");
    }
  }

  const ignoreMenu: MenuProps["items"] = (
    ["once", "by_ext", "by_name", "by_dir"] as InboxIgnoreRuleKind[]
  ).map((kind) => ({
    key: kind,
    label: IGNORE_MENU_LABELS[kind],
    disabled:
      (kind === "by_ext" && !detail.ext) ||
      (kind === "by_name" && !detail.path) ||
      (kind === "by_dir" && !detail.path.includes("/")),
  }));

  return (
    <div style={{ padding: 24 }}>
      <AntSpace direction="vertical" size={16} style={{ width: "100%" }}>
        {/* 敏感文件黄色 Alert（§6.9：不显示预览） */}
        {detail.sensitiveWarning && (
          <Alert
            type="warning"
            showIcon
            message="敏感文件风险"
            description={detail.sensitiveWarning}
          />
        )}

        <Descriptions
          title="条目详情"
          column={1}
          bordered
          size="small"
          items={[
            {
              key: "path",
              label: "完整路径",
              children: (
                <Typography.Text code copyable style={{ fontSize: 12 }}>
                  {detail.path}
                </Typography.Text>
              ),
            },
            {
              key: "type",
              label: "建议类型",
              children: detail.suggestedType
                ? TYPE_LABELS[detail.suggestedType]
                : "未识别",
            },
            {
              key: "size",
              label: "文件大小",
              children: humanSize(detail.sizeBytes),
            },
            {
              key: "discoveredAt",
              label: "发现时间",
              children: `${formatUnixSeconds(detail.discoveredAt)}（${formatRelativeTime(detail.discoveredAt)}）`,
            },
            detail.snoozeNote
              ? {
                  key: "snoozeNote",
                  label: "暂后备注",
                  children: detail.snoozeNote,
                }
              : null,
            detail.remindAt
              ? {
                  key: "remindAt",
                  label: "提醒时间",
                  children: formatUnixSeconds(detail.remindAt),
                }
              : null,
          ].filter((x): x is NonNullable<typeof x> => x !== null)}
        />

        {/* 预览：敏感文件不显示；lines 缺防御（后端契约漂移时降级为不显示预览，不白屏） */}
        {!detail.sensitiveWarning &&
          detail.preview &&
          detail.preview.kind === "text" &&
          Array.isArray(detail.preview.lines) && (
          <div>
            <Typography.Title level={5} style={{ marginTop: 0 }}>
              预览
            </Typography.Title>
            <pre
              style={{
                background: "#f5f5f5",
                padding: 12,
                borderRadius: 6,
                fontSize: 12,
                lineHeight: 1.6,
                maxHeight: 240,
                overflow: "auto",
                margin: 0,
                whiteSpace: "pre-wrap",
                wordBreak: "break-all",
              }}
            >
              {detail.preview.lines.join("\n")}
              {detail.preview.truncated ? "\n…（已截断）" : ""}
            </pre>
          </div>
        )}

        {/* 操作按钮 */}
        <AntSpace wrap>
          <Button
            type="primary"
            icon={<CheckOutlined />}
            disabled={!canOperate || submitting}
            onClick={() => onProcess(detail)}
          >
            处理
          </Button>
          <Button
            icon={<ClockCircleOutlined />}
            disabled={!canOperate || submitting}
            onClick={() => setSnoozeOpen(true)}
          >
            暂后
          </Button>
          <Dropdown
            menu={{
              items: ignoreMenu,
              onClick: ({ key }) => void handleIgnore(key as InboxIgnoreRuleKind),
            }}
            disabled={!canOperate || submitting}
            trigger={["click"]}
          >
            <Button icon={<DeleteOutlined />}>
              忽略 <DownOutlined />
            </Button>
          </Dropdown>
          {/* M7-1 · 文件操作（按 path 直接操作，与是否已处理无关） */}
          <Button icon={<PlayCircleOutlined />} onClick={() => void handleOpenPath()}>
            打开
          </Button>
          <Button icon={<FolderOpenOutlined />} onClick={() => void handleRevealPath()}>
            定位
          </Button>
          <Button icon={<CopyOutlined />} onClick={() => void handleCopyPath()}>
            复制路径
          </Button>
        </AntSpace>
      </AntSpace>

      {/* 暂后 Modal */}
      <Modal
        title="暂后处理"
        open={snoozeOpen}
        onCancel={() => {
          setSnoozeOpen(false);
          snoozeForm.resetFields();
        }}
        onOk={() => void handleSnoozeSubmit()}
        confirmLoading={submitting}
        okText="确定"
        cancelText="取消"
        destroyOnHidden
      >
        <Form form={snoozeForm} layout="vertical">
          <Form.Item name="note" label="备注（可选）">
            <Input.TextArea rows={3} placeholder="例如：等下周例会前再处理" maxLength={200} />
          </Form.Item>
          <Form.Item name="remindAt" label="提醒时间（可选）">
            <DatePicker showTime style={{ width: "100%" }} />
          </Form.Item>
        </Form>
      </Modal>
    </div>
  );
}
