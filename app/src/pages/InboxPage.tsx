import { useCallback, useEffect, useState } from "react";
import {
  Empty,
  Layout,
  List,
  Pagination,
  Tabs,
  Typography,
  message,
} from "antd";
import type {
  InboxItem,
  InboxItemDetail,
  InboxStatus,
} from "../api";
import { inboxGet, inboxList, toApiError } from "../api";
import { InboxItemCard } from "../components/InboxItemCard";
import { InboxDetailPanel } from "../components/InboxDetailPanel";
import { InboxAssignDialog } from "../components/InboxAssignDialog";

/**
 * 收件箱页 — 任务包 m5-5.6 / m5-5.7。
 * 契约：详细设计说明书 §2.7。
 *
 * 布局：左侧列表（Tabs 状态过滤 + List + Pagination pageSize=50），
 * 右侧详情面板（InboxDetailPanel）。
 *
 * 「处理」按钮：m5-5.7 接入 InboxAssignDialog，支持 external/managed 两种模式。
 */

const PAGE_SIZE = 50;

type StatusFilter = InboxStatus | "all";

const TAB_ITEMS: { key: StatusFilter; label: string }[] = [
  { key: "pending", label: "待处理" },
  { key: "snoozed", label: "暂后" },
  { key: "ignored", label: "已忽略" },
  { key: "all", label: "全部" },
];

export function InboxPage() {
  const [status, setStatus] = useState<StatusFilter>("pending");
  const [page, setPage] = useState(1);
  const [items, setItems] = useState<InboxItem[]>([]);
  const [loading, setLoading] = useState(false);
  /** 是否可能还有下一页（当前页返回数量 == PAGE_SIZE 时认为有） */
  const [hasMore, setHasMore] = useState(false);

  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [detail, setDetail] = useState<InboxItemDetail | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);

  // m5-5.7 处理对话框
  const [assignOpen, setAssignOpen] = useState(false);
  const [assignItem, setAssignItem] = useState<InboxItem | null>(null);

  const loadList = useCallback(
    async (nextStatus: StatusFilter, nextPage: number, keepSelection = false) => {
      setLoading(true);
      try {
        const list = await inboxList({
          status: nextStatus === "all" ? undefined : nextStatus,
          limit: PAGE_SIZE,
          offset: (nextPage - 1) * PAGE_SIZE,
        });
        setItems(list);
        setHasMore(list.length === PAGE_SIZE);
        if (!keepSelection) {
          // 切换状态/分页时清空选中
          setSelectedId(null);
          setDetail(null);
        }
      } catch (err) {
        const apiErr = toApiError(err);
        message.error(`加载收件箱失败：${apiErr.message}`);
      } finally {
        setLoading(false);
      }
    },
    [],
  );

  const loadDetail = useCallback(async (id: string) => {
    setDetailLoading(true);
    try {
      const d = await inboxGet(id);
      setDetail(d);
    } catch (err) {
      const apiErr = toApiError(err);
      message.error(`加载详情失败：${apiErr.message}`);
      setDetail(null);
    } finally {
      setDetailLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadList(status, page);
  }, [status, page, loadList]);

  function handleSelect(item: InboxItem) {
    setSelectedId(item.id);
    void loadDetail(item.id);
  }

  function handleTabChange(key: string) {
    setStatus(key as StatusFilter);
    setPage(1);
  }

  /** snooze/ignore 成功后：刷新列表 + 详情 */
  function handleChanged(updated: InboxItem) {
    // 若更新后的 status 不再匹配当前过滤，从列表中移除并清空选中
    if (status !== "all" && updated.status !== status) {
      setItems((prev) => prev.filter((i) => i.id !== updated.id));
      setSelectedId(null);
      setDetail(null);
    } else {
      setItems((prev) => prev.map((i) => (i.id === updated.id ? updated : i)));
      setDetail((prev) => (prev && prev.id === updated.id ? { ...prev, ...updated } : prev));
    }
  }

  /** 「处理」按钮：打开 m5-5.7 处理对话框 */
  function handleProcess(item: InboxItem) {
    setAssignItem(item);
    setAssignOpen(true);
  }

  /** 处理成功后：关闭对话框 + 刷新列表 + 清空选中（条目已转为 processed） */
  function handleAssigned() {
    setAssignOpen(false);
    setAssignItem(null);
    setSelectedId(null);
    setDetail(null);
    void loadList(status, page);
  }

  return (
    <Layout style={{ height: "100%", background: "transparent" }}>
      <Layout.Sider
        width={380}
        style={{
          background: "#fff",
          borderRight: "1px solid #f0f0f0",
          display: "flex",
          flexDirection: "column",
          overflow: "hidden",
        }}
      >
        <div style={{ padding: "16px 16px 0" }}>
          <Typography.Title level={4} style={{ margin: 0, marginBottom: 12, color: "#1e1e1e" }}>
            收件箱
          </Typography.Title>
          <Tabs
            activeKey={status}
            items={TAB_ITEMS}
            onChange={handleTabChange}
            size="small"
          />
        </div>
        <div style={{ flex: 1, overflowY: "auto" }}>
          <List<InboxItem>
            loading={loading}
            dataSource={items}
            rowKey="id"
            locale={{ emptyText: <Empty description="暂无条目" /> }}
            renderItem={(item) => (
              <InboxItemCard
                item={item}
                selected={item.id === selectedId}
                onSelect={handleSelect}
              />
            )}
          />
        </div>
        <div
          style={{
            padding: 12,
            borderTop: "1px solid #f0f0f0",
            display: "flex",
            justifyContent: "flex-end",
            background: "#fff",
          }}
        >
          <Pagination
            current={page}
            pageSize={PAGE_SIZE}
            total={hasMore ? page * PAGE_SIZE + 1 : (page - 1) * PAGE_SIZE + items.length}
            onChange={(p) => setPage(p)}
            showSizeChanger={false}
            size="small"
          />
        </div>
      </Layout.Sider>
      <Layout.Content style={{ overflowY: "auto", background: "#fff" }}>
        <InboxDetailPanel
          detail={detail}
          loading={detailLoading}
          onProcess={handleProcess}
          onChanged={handleChanged}
        />
      </Layout.Content>

      {/* m5-5.7 处理对话框 */}
      <InboxAssignDialog
        open={assignOpen}
        item={assignItem}
        onClose={() => {
          setAssignOpen(false);
          setAssignItem(null);
        }}
        onAssigned={handleAssigned}
      />
    </Layout>
  );
}
