import { useCallback, useEffect, useMemo, useState } from "react";
import { Pagination, Select, Space as AntSpace, Table, Tag, Typography, message } from "antd";
import type { ColumnsType } from "antd/es/table";
import type { DispositionAudit, DispositionAuditAction } from "../api";
import { dispAuditList, toApiError } from "../api";

/**
 * 处置审计页 — 任务包 m4-4.6。
 * 契约：详细设计说明书 §2.6 disp_audit_list / §3.2 disposition_audit 表。
 *
 * 功能：
 * - 列表：时间 / 引用名快照 / 操作（中文 + 颜色）/ 路径快照 / actor / note
 * - 筛选：action 下拉（全部 / 归档 / 恢复 / 删除 / 销毁）
 * - 分页：pageSize 固定 50（与后端 limit 默认值对齐）
 *
 * 关键约束（§6.7）：`disposition_audit` 无外键，销毁后审计独立存活；
 * 因此 refName / locatorSnapshot 直接渲染快照文本，不 JOIN 当前 resource_reference。
 */

const PAGE_SIZE = 50;

const ACTION_LABELS: Record<DispositionAuditAction, string> = {
  archive: "归档",
  unarchive: "恢复",
  soft_delete: "删除",
  destroy: "销毁",
};

const ACTION_COLORS: Record<DispositionAuditAction, string> = {
  archive: "blue",
  unarchive: "green",
  soft_delete: "gold",
  destroy: "red",
};

function formatUnixSeconds(ts: number): string {
  if (!Number.isFinite(ts)) return "-";
  return new Date(ts * 1000).toLocaleString();
}

/** 从 locatorSnapshot 提取可读路径；非 path 类型退化为 JSON 字符串。 */
function formatLocator(audit: DispositionAudit): string {
  const snap = audit.locatorSnapshot;
  if (!snap) return "-";
  if (snap.kind === "path") return snap.path;
  try {
    return JSON.stringify(snap);
  } catch {
    return "-";
  }
}

export function AuditPage() {
  const [action, setAction] = useState<DispositionAuditAction | undefined>(undefined);
  const [page, setPage] = useState(1);
  const [items, setItems] = useState<DispositionAudit[]>([]);
  const [loading, setLoading] = useState(false);
  /** 是否可能还有下一页（当前页返回数量 == PAGE_SIZE 时认为有） */
  const [hasMore, setHasMore] = useState(false);

  const load = useCallback(
    async (nextPage: number, nextAction: DispositionAuditAction | undefined) => {
      setLoading(true);
      try {
        const list = await dispAuditList({
          action: nextAction,
          limit: PAGE_SIZE,
          offset: (nextPage - 1) * PAGE_SIZE,
        });
        setItems(list);
        setHasMore(list.length === PAGE_SIZE);
      } catch (err) {
        const apiErr = toApiError(err);
        message.error(`加载审计失败：${apiErr.message}`);
      } finally {
        setLoading(false);
      }
    },
    [],
  );

  useEffect(() => {
    void load(page, action);
  }, [page, action, load]);

  const columns: ColumnsType<DispositionAudit> = useMemo(
    () => [
      {
        title: "时间",
        dataIndex: "at",
        key: "at",
        width: 180,
        render: (at: number) => formatUnixSeconds(at),
      },
      {
        title: "引用名",
        dataIndex: "refName",
        key: "refName",
        ellipsis: true,
        render: (name: string) => <Typography.Text>{name}</Typography.Text>,
      },
      {
        title: "操作",
        dataIndex: "action",
        key: "action",
        width: 100,
        render: (a: DispositionAuditAction) => (
          <Tag color={ACTION_COLORS[a]}>{ACTION_LABELS[a]}</Tag>
        ),
      },
      {
        title: "路径快照",
        key: "locator",
        ellipsis: true,
        render: (_, record) => (
          <Typography.Text code style={{ fontSize: 12 }}>
            {formatLocator(record)}
          </Typography.Text>
        ),
      },
      {
        title: "操作者",
        dataIndex: "actor",
        key: "actor",
        width: 120,
      },
      {
        title: "备注",
        dataIndex: "note",
        key: "note",
        ellipsis: true,
        render: (note: string | null | undefined) =>
          note ? (
            <Typography.Text type="secondary" style={{ fontSize: 12 }}>
              {note}
            </Typography.Text>
          ) : (
            <Typography.Text type="secondary">-</Typography.Text>
          ),
      },
    ],
    [],
  );

  return (
    <div style={{ padding: 24 }}>
      <AntSpace style={{ marginBottom: 16, width: "100%", justifyContent: "space-between" }}>
        <Typography.Title level={4} style={{ margin: 0 }}>
          处置审计
        </Typography.Title>
        <Select<DispositionAuditAction | undefined>
          value={action}
          onChange={(v) => {
            setAction(v);
            setPage(1);
          }}
          style={{ width: 160 }}
          placeholder="按操作筛选"
          allowClear
          options={[
            { value: undefined, label: "全部" },
            { value: "archive", label: "归档" },
            { value: "unarchive", label: "恢复" },
            { value: "soft_delete", label: "删除" },
            { value: "destroy", label: "销毁" },
          ]}
        />
      </AntSpace>

      <Table<DispositionAudit>
        rowKey="id"
        columns={columns}
        dataSource={items}
        loading={loading}
        pagination={false}
        size="middle"
      />

      <div style={{ marginTop: 16, display: "flex", justifyContent: "flex-end" }}>
        <Pagination
          current={page}
          pageSize={PAGE_SIZE}
          total={hasMore ? page * PAGE_SIZE + 1 : (page - 1) * PAGE_SIZE + items.length}
          onChange={(p) => setPage(p)}
          showSizeChanger={false}
        />
      </div>
    </div>
  );
}
