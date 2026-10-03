import { List, Space as AntSpace, Tag, Typography } from "antd";
import type { InboxItem, InboxStatus, ReferenceType } from "../api";

/**
 * 收件箱列表项 — 任务包 m5-5.6。
 * 显示：文件名 / 建议类型 / 相对发现时间 / 状态标签。
 * 点击选中，由父组件控制高亮。
 */

const STATUS_LABELS: Record<InboxStatus, string> = {
  pending: "待处理",
  snoozed: "暂后",
  processed: "已处理",
  ignored: "已忽略",
  stale: "已失效",
};

const STATUS_COLORS: Record<InboxStatus, string> = {
  pending: "blue",
  snoozed: "gold",
  processed: "green",
  ignored: "default",
  stale: "red",
};

const TYPE_LABELS: Record<ReferenceType, string> = {
  code: "代码",
  document: "文档",
  data: "数据",
  artifact: "制品",
  tool: "工具",
  media: "媒体",
};

/** 相对时间格式化：3 分钟前 / 2 小时前 / 1 天前 / 具体日期 */
export function formatRelativeTime(unixSeconds: number): string {
  if (!Number.isFinite(unixSeconds)) return "-";
  const now = Math.floor(Date.now() / 1000);
  const diff = now - unixSeconds;
  if (diff < 60) return "刚刚";
  if (diff < 3600) return `${Math.floor(diff / 60)} 分钟前`;
  if (diff < 86400) return `${Math.floor(diff / 3600)} 小时前`;
  if (diff < 86400 * 7) return `${Math.floor(diff / 86400)} 天前`;
  return new Date(unixSeconds * 1000).toLocaleDateString();
}

function fileName(path: string): string {
  const segs = path.split("/").filter(Boolean);
  return segs[segs.length - 1] ?? path;
}

interface InboxItemCardProps {
  item: InboxItem;
  selected: boolean;
  onSelect: (item: InboxItem) => void;
}

export function InboxItemCard({ item, selected, onSelect }: InboxItemCardProps) {
  return (
    <List.Item
      onClick={() => onSelect(item)}
      style={{
        cursor: "pointer",
        padding: "12px 16px",
        background: selected ? "rgba(74, 144, 217, 0.08)" : undefined,
        borderLeft: selected ? "3px solid #4a90d9" : "3px solid transparent",
      }}
    >
      <div style={{ width: "100%", minWidth: 0 }}>
        {/* 文件名行：AntSpace 会包一层 .ant-space-item 不定宽，ellipsis 拿不到宽度约束；
            改用 flex div，Text 作为直接 flex 子项（minWidth:0 允许收缩），超长省略号 + 悬浮显示全名 */}
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 8 }}>
          <Typography.Text
            strong
            ellipsis={{ tooltip: fileName(item.path) }}
            style={{ flex: 1, minWidth: 0 }}
          >
            {fileName(item.path)}
          </Typography.Text>
          <Tag color={STATUS_COLORS[item.status]} style={{ marginInlineEnd: 0, flexShrink: 0 }}>
            {STATUS_LABELS[item.status]}
          </Tag>
        </div>
        <AntSpace size={8} style={{ marginTop: 4, fontSize: 12 }}>
          {item.suggestedType ? (
            <Tag style={{ marginInlineEnd: 0 }}>{TYPE_LABELS[item.suggestedType]}</Tag>
          ) : (
            <Typography.Text type="secondary" style={{ fontSize: 12 }}>
              未识别
            </Typography.Text>
          )}
          <Typography.Text type="secondary" style={{ fontSize: 12 }}>
            {formatRelativeTime(item.discoveredAt)}
          </Typography.Text>
        </AntSpace>
      </div>
    </List.Item>
  );
}
