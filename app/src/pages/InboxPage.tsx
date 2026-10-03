import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  Alert,
  Button,
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
import { inboxDismissAllStale, inboxGet, inboxList, inboxStats, toApiError } from "../api";
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

  /** 已失效（stale）条目数：> 0 时显示批量清理提示条 */
  const [staleCount, setStaleCount] = useState(0);
  const [cleaningStale, setCleaningStale] = useState(false);

  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [detail, setDetail] = useState<InboxItemDetail | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);

  // m5-5.7 处理对话框
  const [assignOpen, setAssignOpen] = useState(false);
  const [assignItem, setAssignItem] = useState<InboxItem | null>(null);

  const refreshStaleCount = useCallback(async () => {
    try {
      const s = await inboxStats();
      setStaleCount(s.stale);
    } catch {
      // 统计失败不阻塞列表；保持旧值
    }
  }, []);

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
        void refreshStaleCount();
      } catch (err) {
        const apiErr = toApiError(err);
        message.error(`加载收件箱失败：${apiErr.message}`);
      } finally {
        setLoading(false);
      }
    },
    [refreshStaleCount],
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

  /**
   * 事件驱动的即时刷新：后端聚合窗口有实际写入时 emit "inbox-changed"，
   * 本页监听后静默重载当前列表（keepSelection=true，不打断用户正在查看的详情）。
   * 用 ref 持有最新刷新闭包，避免 status/page 变化时反复解绑/重绑监听。
   */
  const silentRefreshRef = useRef<() => void>(() => {});
  silentRefreshRef.current = () => void loadList(status, page, true);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let disposed = false;
    void listen("inbox-changed", () => silentRefreshRef.current()).then((u) => {
      if (disposed) u();
      else unlisten = u;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

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

  /** 「全部清理」：把所有 stale 条目批量置为已处理 */
  async function handleCleanupStale() {
    setCleaningStale(true);
    try {
      const dismissed = await inboxDismissAllStale();
      message.success(`已清理 ${dismissed} 条失效条目`);
      setStaleCount(0);
      // 若当前正在 stale 视图，清理后切回待处理
      if (status === "stale") {
        setStatus("pending");
        setPage(1);
      } else {
        void loadList(status, page, true);
      }
    } catch (err) {
      const apiErr = toApiError(err);
      message.error(`清理失效条目失败：${apiErr.message}`);
    } finally {
      setCleaningStale(false);
    }
  }

  /** staleCount > 0 时追加「已失效」Tab（默认隐藏，仅通过提示条/统计触达） */
  const tabItems =
    staleCount > 0
      ? [...TAB_ITEMS, { key: "stale" as StatusFilter, label: `已失效(${staleCount})` }]
      : TAB_ITEMS;

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
          <Typography.Title level={4} style={{ margin: 0, marginBottom: 12 }}>
            收件箱
          </Typography.Title>
          <Tabs
            activeKey={status}
            items={tabItems}
            onChange={handleTabChange}
            size="small"
          />
          {staleCount > 0 && status !== "stale" && (
            <Alert
              type="warning"
              showIcon
              style={{ marginBottom: 8 }}
              message={`有 ${staleCount} 条已失效条目（源文件已不在）`}
              action={
                <>
                  <Button size="small" type="link" onClick={() => handleTabChange("stale")}>
                    查看
                  </Button>
                  <Button
                    size="small"
                    type="link"
                    loading={cleaningStale}
                    onClick={handleCleanupStale}
                  >
                    全部清理
                  </Button>
                </>
              }
            />
          )}
          {status === "stale" && (
            <Alert
              type="warning"
              showIcon
              style={{ marginBottom: 8 }}
              message="以下为已失效条目（源文件已不在），可批量清理为已处理"
              action={
                <Button
                  size="small"
                  type="link"
                  loading={cleaningStale}
                  onClick={handleCleanupStale}
                >
                  全部清理
                </Button>
              }
            />
          )}
        </div>
        {/*
          minHeight: 0 是必须的：flex 列容器内 flex item 默认 min-height:auto，
          会被内容撑高超出 Sider（overflow:hidden 裁掉），overflowY 永远不触发，
          表现为「列表不能滚动、分页栏被顶出可视区」。与 TerminalPanel 同款处理。
        */}
        <div style={{ flex: 1, minHeight: 0, overflowY: "auto" }}>
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
