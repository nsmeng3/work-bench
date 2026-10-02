import { useEffect, useState } from "react";
import { Card, Col, Row, Spin, Statistic, message } from "antd";
import { statsOverview, toApiError, type StatsOverview } from "../api";

const TYPE_LABEL: Record<string, string> = {
  code: "代码",
  document: "文档",
  data: "数据",
  artifact: "产物",
  tool: "工具",
  media: "媒体",
};

/** 纯 CSS 条形图（不引图表库）：值相对最大值按比例着色。 */
function MiniBar({ label, value, max }: { label: string; value: number; max: number }) {
  const pct = max > 0 ? Math.max((value / max) * 100, value > 0 ? 4 : 0) : 0;
  return (
    <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 8 }}>
      <span style={{ width: 56, fontSize: 12, color: "#666", textAlign: "right" }}>{label}</span>
      <div style={{ flex: 1, background: "#f0f0f0", borderRadius: 3, height: 14 }}>
        <div
          style={{
            width: `${pct}%`,
            height: "100%",
            background: "#4a90d9",
            borderRadius: 3,
            transition: "width 0.3s",
          }}
        />
      </div>
      <span style={{ width: 36, fontSize: 12, color: "#333" }}>{value}</span>
    </div>
  );
}

/**
 * m7-7.6 · 工作台统计页：总量卡片 + 引用类型分布 + 近 7 天访问趋势。
 * 数据来自 `stats_overview` 单命令聚合。
 */
export function StatsPage() {
  const [data, setData] = useState<StatsOverview | null>(null);
  const [messageApi, messageContextHolder] = message.useMessage();

  useEffect(() => {
    statsOverview()
      .then(setData)
      .catch((err) => {
        const apiErr = toApiError(err);
        messageApi.error({ content: `加载统计失败：${apiErr.message}`, duration: 3 });
      });
  }, [messageApi]);

  if (!data) {
    return (
      <div style={{ paddingTop: 80, textAlign: "center" }}>
        {messageContextHolder}
        <Spin size="large" tip="正在统计…" />
      </div>
    );
  }

  const maxType = Math.max(1, ...data.refsByType.map((t) => t.count));
  const maxDay = Math.max(1, ...data.accessLast7d.map((d) => d.count));

  return (
    <div style={{ maxWidth: 960 }}>
      {messageContextHolder}
      <h1 style={{ fontSize: 20, margin: 0, marginBottom: 16 }}>统计</h1>

      <Row gutter={[16, 16]}>
        <Col span={6}>
          <Card><Statistic title="空间" value={data.spaceCount} /></Card>
        </Col>
        <Col span={6}>
          <Card><Statistic title="资源集" value={data.collectionCount} /></Card>
        </Col>
        <Col span={6}>
          <Card><Statistic title="资源引用" value={data.referenceCount} /></Card>
        </Col>
        <Col span={6}>
          <Card><Statistic title="监控目录" value={data.watchDirCount} /></Card>
        </Col>
        <Col span={6}>
          <Card><Statistic title="待办（待处理）" value={data.todoPending} /></Card>
        </Col>
        <Col span={6}>
          <Card><Statistic title="待办（进行中）" value={data.todoDoing} /></Card>
        </Col>
        <Col span={6}>
          <Card><Statistic title="今日完成" value={data.todoDoneToday} /></Card>
        </Col>
        <Col span={6}>
          <Card>
            <Statistic
              title="收件箱待处理"
              value={data.inboxPending}
              suffix={data.inboxSnoozed > 0 ? `（搁置 ${data.inboxSnoozed}）` : undefined}
            />
          </Card>
        </Col>
      </Row>

      <Row gutter={[16, 16]} style={{ marginTop: 16 }}>
        <Col span={12}>
          <Card title="引用类型分布" size="small">
            {data.refsByType.length === 0 ? (
              <span style={{ color: "#999", fontSize: 12 }}>暂无引用</span>
            ) : (
              data.refsByType.map((t) => (
                <MiniBar
                  key={t.type}
                  label={TYPE_LABEL[t.type] ?? t.type}
                  value={t.count}
                  max={maxType}
                />
              ))
            )}
          </Card>
        </Col>
        <Col span={12}>
          <Card title="近 7 天资源访问" size="small">
            {data.accessLast7d.map((d) => (
              <MiniBar key={d.date} label={d.date} value={d.count} max={maxDay} />
            ))}
          </Card>
        </Col>
      </Row>
    </div>
  );
}
