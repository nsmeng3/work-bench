import { useCallback, useEffect, useMemo, useState } from "react";
import {
  Badge,
  Button,
  Card,
  Col,
  Empty,
  List,
  Row,
  Space as AntSpace,
  Spin,
  Tag,
  Typography,
  message,
} from "antd";
import {
  AppstoreOutlined,
  CheckSquareOutlined,
  ClockCircleOutlined,
  FileTextOutlined,
  FolderOutlined,
  InboxOutlined,
  LinkOutlined,
  RightOutlined,
  RocketOutlined,
} from "@ant-design/icons";
import type { Collection, RecentRef, Space, Todo } from "../api";
import {
  collectionList,
  inboxStats,
  refOpen,
  refRecentAccess,
  settingsGetUserName,
  spaceList,
  todoToday,
  toApiError,
} from "../api";

const { Title, Text } = Typography;

/** 问候语按小时切换（任务包 m7-7.3 §前端） */
function greetingByHour(hour: number): string {
  if (hour >= 6 && hour < 12) return "早安";
  if (hour >= 12 && hour < 18) return "下午好";
  if (hour >= 18 && hour < 24) return "晚上好";
  return "夜深了";
}

/** 周几中文 */
const WEEKDAYS = ["周日", "周一", "周二", "周三", "周四", "周五", "周六"];

/** 相对时间（"2 小时前"） */
function formatRelativeTime(unixSec: number): string {
  const now = Math.floor(Date.now() / 1000);
  const diff = now - unixSec;
  if (diff < 60) return "刚刚";
  if (diff < 3600) return `${Math.floor(diff / 60)} 分钟前`;
  if (diff < 86400) return `${Math.floor(diff / 3600)} 小时前`;
  if (diff < 86400 * 2) return "昨天";
  if (diff < 86400 * 7) return `${Math.floor(diff / 86400)} 天前`;
  const d = new Date(unixSec * 1000);
  const month = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${month}-${day}`;
}

/** 优先级 → 中文 + 颜色（与 TodoListPanel 对齐） */
const PRIORITY_META: Record<number, { label: string; color?: string }> = {
  0: { label: "普通" },
  1: { label: "重要", color: "orange" },
  2: { label: "紧急", color: "red" },
};

/** 引用类型 → 图标（简化版） */
function refTypeIcon(refType: string) {
  switch (refType) {
    case "code":
      return <FolderOutlined />;
    case "document":
      return <FileTextOutlined />;
    case "media":
      return <FileTextOutlined />;
    default:
      return <LinkOutlined />;
  }
}

/** 从 locator_json 提取路径（用于副标题展示） */
function locatorPathOf(locatorJson: string): string | null {
  try {
    const v = JSON.parse(locatorJson) as { kind?: string; path?: string };
    if (v && v.kind === "path" && typeof v.path === "string") return v.path;
    return null;
  } catch {
    return null;
  }
}

export interface DashboardPageProps {
  /** 点击"查看全部待办"跳转 TodoPage */
  onGoTodo: () => void;
  /** 点击"收件箱"卡片跳转 InboxPage */
  onGoInbox: () => void;
  /** 点击空间卡片下钻 */
  onEnterSpace: (space: Space) => void;
}

/**
 * Dashboard 首页（M7-3 · 任务包 m7-7.3）。
 *
 * 布局：
 * - 顶部问候条（按小时切换 + 用户名 + 日期）
 * - 左：今日待办（todo_today，最多 5 条 + 查看全部）
 * - 右：最近资源（ref_recent_access，最多 8 条，点击调 refOpen）
 * - 下：我的空间（active 空间卡片网格，点击下钻）
 * - 底：收件箱提示（pending ≥ 1 时显示）
 */
export function DashboardPage({ onGoTodo, onGoInbox, onEnterSpace }: DashboardPageProps) {
  const [userName, setUserName] = useState<string | null>(null);
  const [todos, setTodos] = useState<Todo[]>([]);
  const [todosLoading, setTodosLoading] = useState(true);
  const [recents, setRecents] = useState<RecentRef[]>([]);
  const [recentsLoading, setRecentsLoading] = useState(true);
  const [spaces, setSpaces] = useState<Space[]>([]);
  const [spacesLoading, setSpacesLoading] = useState(true);
  const [collectionCountBySpace, setCollectionCountBySpace] = useState<Record<string, number>>({});
  const [inboxPending, setInboxPending] = useState(0);

  const [messageApi, messageContextHolder] = message.useMessage();

  // 顶部问候
  const now = new Date();
  const greeting = greetingByHour(now.getHours());
  const dateStr = useMemo(() => {
    const month = String(now.getMonth() + 1).padStart(2, "0");
    const day = String(now.getDate()).padStart(2, "0");
    return `今天 ${month}-${day} ${WEEKDAYS[now.getDay()]}`;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // 加载用户名（失败静默 → 显示"朋友"）
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const cfg = await settingsGetUserName();
        if (!cancelled) setUserName(cfg.userName ?? null);
      } catch (err) {
        console.warn("[Dashboard] 加载用户名失败", err);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  // 加载今日待办
  const fetchTodos = useCallback(async () => {
    setTodosLoading(true);
    try {
      const list = await todoToday();
      setTodos(list);
    } catch (err) {
      const apiErr = toApiError(err);
      messageApi.error({ content: `加载今日待办失败：${apiErr.message}`, duration: 3 });
    } finally {
      setTodosLoading(false);
    }
  }, [messageApi]);

  // 加载最近资源
  const fetchRecents = useCallback(async () => {
    setRecentsLoading(true);
    try {
      const list = await refRecentAccess(8);
      setRecents(list);
    } catch (err) {
      const apiErr = toApiError(err);
      messageApi.error({ content: `加载最近资源失败：${apiErr.message}`, duration: 3 });
    } finally {
      setRecentsLoading(false);
    }
  }, [messageApi]);

  // 加载空间 + 每个空间的资源集数
  const fetchSpaces = useCallback(async () => {
    setSpacesLoading(true);
    try {
      const list = await spaceList({ status: "active" });
      setSpaces(list);
      // 并发拉每个空间的资源集数（失败仅 console.warn，不阻塞主卡片）
      const counts = await Promise.all(
        list.map(async (s) => {
          try {
            const cols: Collection[] = await collectionList({ spaceId: s.id, status: "active" });
            return [s.id, cols.length] as const;
          } catch (err) {
            console.warn("[Dashboard] 加载空间资源集数失败", s.id, err);
            return [s.id, 0] as const;
          }
        }),
      );
      setCollectionCountBySpace(Object.fromEntries(counts));
    } catch (err) {
      const apiErr = toApiError(err);
      messageApi.error({ content: `加载空间失败：${apiErr.message}`, duration: 3 });
    } finally {
      setSpacesLoading(false);
    }
  }, [messageApi]);

  // 加载收件箱统计（一次性；角标轮询由 App.tsx 的 useInboxStats 负责）
  const fetchInboxStats = useCallback(async () => {
    try {
      const stats = await inboxStats();
      setInboxPending(stats.pending);
    } catch (err) {
      console.warn("[Dashboard] 加载收件箱统计失败", err);
    }
  }, []);

  useEffect(() => {
    void fetchTodos();
    void fetchRecents();
    void fetchSpaces();
    void fetchInboxStats();
  }, [fetchTodos, fetchRecents, fetchSpaces, fetchInboxStats]);

  /** 点击最近资源：调 refOpen 打开（M7-1 API） */
  async function handleOpenRecent(item: RecentRef) {
    try {
      await refOpen(item.refId);
      // refOpen 后端已自动写 access_log；前端无需重复埋点
    } catch (err) {
      const apiErr = toApiError(err);
      messageApi.error({ content: `打开失败：${apiErr.message}`, duration: 3 });
    }
  }

  const displayName = userName?.trim() || "朋友";
  const topTodos = todos.slice(0, 5);

  return (
    <div style={{ maxWidth: 1200, margin: "0 auto" }}>
      {messageContextHolder}

      {/* 顶部问候条 */}
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "baseline",
          marginBottom: 24,
          flexWrap: "wrap",
          gap: 8,
        }}
      >
        <Title level={2} style={{ margin: 0 }}>
          {greeting}，{displayName}
        </Title>
        <Text type="secondary">{dateStr}</Text>
      </div>

      {/* 上排：今日待办 + 最近资源 */}
      <Row gutter={[16, 16]}>
        <Col xs={24} lg={12}>
          <Card
            title={
              <AntSpace>
                <CheckSquareOutlined />
                <span>今日待办</span>
                <Text type="secondary" style={{ fontSize: 12, fontWeight: "normal" }}>
                  ({todos.length})
                </Text>
              </AntSpace>
            }
            extra={
              <Button type="link" size="small" onClick={onGoTodo}>
                查看全部 <RightOutlined />
              </Button>
            }
            loading={todosLoading && todos.length === 0}
          >
            {topTodos.length === 0 ? (
              <Empty description="今日暂无待办" image={Empty.PRESENTED_IMAGE_SIMPLE} />
            ) : (
              <List
                size="small"
                dataSource={topTodos}
                renderItem={(todo) => {
                  const priorityMeta = PRIORITY_META[todo.priority] ?? PRIORITY_META[0];
                  return (
                    <List.Item style={{ padding: "8px 0", display: "block" }}>
                      <div style={{ display: "flex", alignItems: "flex-start", gap: 8, width: "100%", overflow: "hidden" }}>
                        <div
                          style={{
                            fontWeight: 500,
                            flex: 1,
                            minWidth: 0,
                            display: "-webkit-box",
                            WebkitBoxOrient: "vertical",
                            WebkitLineClamp: 2,
                            overflow: "hidden",
                            wordBreak: "break-all",
                            lineHeight: 1.4,
                          }}
                          title={todo.title}
                        >
                          {todo.title}
                        </div>
                        {todo.priority > 0 && (
                          <Tag color={priorityMeta.color} style={{ marginInlineEnd: 0, flexShrink: 0, marginTop: 2 }}>
                            {priorityMeta.label}
                          </Tag>
                        )}
                        {todo.status === "doing" && <Tag color="processing" style={{ flexShrink: 0, marginTop: 2 }}>进行中</Tag>}
                        {todo.dueAt && (
                          <span style={{ fontSize: 12, color: "rgba(0, 0, 0, 0.45)", flexShrink: 0, marginTop: 2 }}>
                            <ClockCircleOutlined /> {formatRelativeTime(todo.dueAt)}
                          </span>
                        )}
                      </div>
                    </List.Item>
                  );
                }}
              />
            )}
          </Card>
        </Col>

        <Col xs={24} lg={12}>
          <Card
            title={
              <AntSpace>
                <ClockCircleOutlined />
                <span>最近资源</span>
              </AntSpace>
            }
            loading={recentsLoading && recents.length === 0}
          >
            {recents.length === 0 ? (
              <Empty
                description="暂无最近访问，去空间页打开几个资源试试"
                image={Empty.PRESENTED_IMAGE_SIMPLE}
              />
            ) : (
              <List
                size="small"
                dataSource={recents}
                renderItem={(item) => {
                  const path = locatorPathOf(item.locatorJson);
                  return (
                    <List.Item
                      style={{ padding: "8px 0", cursor: "pointer", display: "block" }}
                      onClick={() => void handleOpenRecent(item)}
                    >
                      <div style={{ display: "flex", alignItems: "flex-start", gap: 8, width: "100%", overflow: "hidden" }}>
                        <span style={{ flexShrink: 0, marginTop: 2 }}>{refTypeIcon(item.refType)}</span>
                        <div style={{ flex: 1, minWidth: 0, overflow: "hidden" }}>
                          <div
                            style={{
                              fontWeight: 500,
                              display: "-webkit-box",
                              WebkitBoxOrient: "vertical",
                              WebkitLineClamp: 2,
                              overflow: "hidden",
                              wordBreak: "break-all",
                              lineHeight: 1.4,
                            }}
                            title={item.refName}
                          >
                            {item.refName}
                          </div>
                          {path && (
                            <div
                              style={{
                                fontSize: 12,
                                color: "rgba(0, 0, 0, 0.45)",
                                overflow: "hidden",
                                textOverflow: "ellipsis",
                                whiteSpace: "nowrap",
                                marginTop: 2,
                              }}
                              title={path}
                            >
                              {path}
                            </div>
                          )}
                        </div>
                        <span
                          style={{
                            fontSize: 12,
                            color: "rgba(0, 0, 0, 0.45)",
                            flexShrink: 0,
                            marginTop: 2,
                          }}
                        >
                          {formatRelativeTime(item.lastAt)}
                        </span>
                      </div>
                    </List.Item>
                  );
                }}
              />
            )}
          </Card>
        </Col>
      </Row>

      {/* 中排：我的空间 */}
      <Card
        title={
          <AntSpace>
            <RocketOutlined />
            <span>我的空间</span>
          </AntSpace>
        }
        style={{ marginTop: 16 }}
        loading={spacesLoading && spaces.length === 0}
      >
        {spaces.length === 0 ? (
          <Empty description="暂无空间" image={Empty.PRESENTED_IMAGE_SIMPLE} />
        ) : (
          <Row gutter={[12, 12]}>
            {spaces.map((space) => (
              <Col xs={24} sm={12} md={8} lg={6} key={space.id}>
                <Card
                  size="small"
                  hoverable
                  onClick={() => onEnterSpace(space)}
                  style={{ height: "100%" }}
                >
                  <AntSpace direction="vertical" size={4} style={{ width: "100%" }}>
                    <AntSpace>
                      <AppstoreOutlined style={{ color: space.color || "#4a90d9" }} />
                      <Text strong>{space.name}</Text>
                    </AntSpace>
                    {space.description && (
                      <Text
                        type="secondary"
                        style={{
                          fontSize: 12,
                          display: "-webkit-box",
                          WebkitLineClamp: 2,
                          WebkitBoxOrient: "vertical",
                          overflow: "hidden",
                        }}
                      >
                        {space.description}
                      </Text>
                    )}
                    <Text type="secondary" style={{ fontSize: 12 }}>
                      {collectionCountBySpace[space.id] ?? 0} 个资源集
                    </Text>
                  </AntSpace>
                </Card>
              </Col>
            ))}
          </Row>
        )}
      </Card>

      {/* 底排：收件箱提示 */}
      {inboxPending >= 1 && (
        <Card
          size="small"
          style={{ marginTop: 16, cursor: "pointer", borderColor: "#faad14" }}
          onClick={onGoInbox}
        >
          <AntSpace>
            <Badge count={inboxPending} size="small">
              <InboxOutlined style={{ fontSize: 18, color: "#faad14" }} />
            </Badge>
            <Text>
              收件箱有 <Text strong>{inboxPending}</Text> 条待处理
            </Text>
            <RightOutlined style={{ color: "#999" }} />
          </AntSpace>
        </Card>
      )}

      {/* 底部加载占位（任意一个还在加载时显示 Spin） */}
      {(todosLoading || recentsLoading || spacesLoading) && (
        <div style={{ textAlign: "center", padding: "12px 0", color: "#999" }}>
          <Spin size="small" /> <Text type="secondary" style={{ marginLeft: 8, fontSize: 12 }}>加载中…</Text>
        </div>
      )}
    </div>
  );
}
