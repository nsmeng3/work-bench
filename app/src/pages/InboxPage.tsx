import { useCallback, useEffect, useRef, useState } from "react";
import type { UIEvent } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  Alert,
  Button,
  Empty,
  Layout,
  List,
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
 * 布局：左侧列表（Tabs 状态过滤 + List 无限滚动，每批 50 条），
 * 右侧详情面板（InboxDetailPanel）。
 *
 * 「处理」按钮：m5-5.7 接入 InboxAssignDialog，支持 external/managed 两种模式。
 */

/** 每批加载条数（与后端 LIST_MAX_LIMIT=200 对齐：静默刷新最多一次拉回 200） */
const PAGE_SIZE = 50;
/** 后端 limit 上限（inbox.rs LIST_MAX_LIMIT），静默刷新时的封顶 */
const MAX_REFRESH_LIMIT = 200;
/** 距底部多少像素内触发加载下一批 */
const LOAD_MORE_THRESHOLD_PX = 80;

type StatusFilter = InboxStatus | "all";

const TAB_ITEMS: { key: StatusFilter; label: string }[] = [
  { key: "pending", label: "待处理" },
  { key: "snoozed", label: "暂后" },
  { key: "ignored", label: "已忽略" },
  { key: "all", label: "全部" },
];

export function InboxPage() {
  const [status, setStatus] = useState<StatusFilter>("pending");
  const [items, setItems] = useState<InboxItem[]>([]);
  const [loading, setLoading] = useState(false);
  /** 底部追加加载中（区别于首屏 loading，避免整列表闪 loading 态） */
  const [loadingMore, setLoadingMore] = useState(false);
  /** 是否可能还有下一批（当前批返回数量 == PAGE_SIZE 时认为有） */
  const [hasMore, setHasMore] = useState(false);

  /** 列表滚动容器 ref（切 Tab 后回滚顶部） */
  const listRef = useRef<HTMLDivElement>(null);
  /** items 的 ref 镜像：scroll 事件闭包里读最新条数，避免 stale closure */
  const itemsRef = useRef<InboxItem[]>([]);
  itemsRef.current = items;

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

  /**
   * 首屏 / 切 Tab / 静默刷新：从 offset 0 重拉（后端 SQL 级 LIMIT/OFFSET 分页）。
   * limit 默认 PAGE_SIZE；静默刷新时传「已加载条数」（封顶 MAX_REFRESH_LIMIT），
   * 避免已展开的列表被刷掉。
   */
  const loadList = useCallback(
    async (nextStatus: StatusFilter, keepSelection = false, limit: number = PAGE_SIZE) => {
      setLoading(true);
      try {
        const list = await inboxList({
          status: nextStatus === "all" ? undefined : nextStatus,
          limit,
          offset: 0,
        });
        setItems(list);
        setHasMore(list.length === limit);
        if (!keepSelection) {
          // 切换状态时清空选中并回滚列表到顶部
          setSelectedId(null);
          setDetail(null);
          listRef.current?.scrollTo({ top: 0 });
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

  /** 无限滚动：按 offset=已加载条数 向后端拉下一批，追加到列表尾部 */
  const loadMore = useCallback(async () => {
    if (loading || loadingMore || !hasMore) return;
    setLoadingMore(true);
    try {
      const list = await inboxList({
        status: status === "all" ? undefined : status,
        limit: PAGE_SIZE,
        offset: itemsRef.current.length,
      });
      setItems((prev) => [...prev, ...list]);
      setHasMore(list.length === PAGE_SIZE);
    } catch (err) {
      const apiErr = toApiError(err);
      message.error(`加载更多失败：${apiErr.message}`);
    } finally {
      setLoadingMore(false);
    }
  }, [status, loading, loadingMore, hasMore]);

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
    void loadList(status);
  }, [status, loadList]);

  /**
   * 事件驱动的即时刷新：后端聚合窗口有实际写入时 emit "inbox-changed"，
   * 本页监听后静默重载当前列表（keepSelection=true，不打断用户正在查看的详情；
   * limit 取已加载条数，保住无限滚动已展开的列表）。
   * 用 ref 持有最新刷新闭包，避免 status 变化时反复解绑/重绑监听。
   */
  const silentRefreshRef = useRef<() => void>(() => {});
  silentRefreshRef.current = () =>
    void loadList(
      status,
      true,
      Math.min(Math.max(itemsRef.current.length, PAGE_SIZE), MAX_REFRESH_LIMIT),
    );

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
  }

  /** 滚动到底部阈值内时加载下一批（并发/越界由 loadMore 内部守卫） */
  function handleListScroll(e: UIEvent<HTMLDivElement>) {
    const el = e.currentTarget;
    if (el.scrollTop + el.clientHeight >= el.scrollHeight - LOAD_MORE_THRESHOLD_PX) {
      void loadMore();
    }
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
    void loadList(status);
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
      } else {
        void loadList(status, true);
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
        className="inbox-sider"
        style={{
          background: "#fff",
          borderRight: "1px solid #f0f0f0",
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
        <div
          ref={listRef}
          onScroll={handleListScroll}
          style={{ flex: 1, minHeight: 0, overflowY: "auto" }}
        >
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
        {/* 底部加载状态条（取代原分页器）：无限滚动加载提示 */}
        {items.length > 0 && (
          <div
            style={{
              padding: "8px 12px",
              borderTop: "1px solid #f0f0f0",
              textAlign: "center",
              fontSize: 12,
              color: "#999",
              background: "#fff",
            }}
          >
            {loadingMore
              ? "加载中…"
              : hasMore
                ? "滚动到底自动加载更多"
                : `共 ${items.length} 条，已全部加载`}
          </div>
        )}
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
