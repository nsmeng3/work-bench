import { useState, useEffect, useCallback } from "react";
import {
  Badge,
  Button,
  Collapse,
  Descriptions,
  Empty,
  List,
  Space as AntSpace,
  Spin,
  Tag,
  Typography,
  message,
} from "antd";
import { ArrowLeftOutlined, ReloadOutlined } from "@ant-design/icons";
import type {
  Collection,
  CollectionDetail,
  ReferenceHealth,
  ReferenceType,
  ReferenceWithHealth,
  Space,
} from "../api";
import { collectionGet, toApiError } from "../api";

const { Text, Paragraph } = Typography;

/** 六类型分组固定键序 — 详细设计说明书 §2.4 */
const TYPE_ORDER: ReferenceType[] = ["code", "document", "data", "artifact", "tool", "media"];

const TYPE_LABEL: Record<ReferenceType, string> = {
  code: "代码",
  document: "文档",
  data: "数据",
  artifact: "构建产物",
  tool: "工具",
  media: "媒体",
};

const HEALTH_META: Record<ReferenceHealth, { color: string; text: string }> = {
  ok: { color: "success", text: "正常" },
  missing: { color: "error", text: "失效" },
  unknown: { color: "default", text: "未检测" },
};

const LIFECYCLE_LABEL: Record<string, string> = {
  active: "活跃",
  staged: "已暂存",
  delivered: "已交付",
  archived: "已归档",
};

const CONFIDENTIALITY_LABEL: Record<string, string> = {
  public: "公开",
  internal: "内部",
  customer_restricted: "客户受限",
  sensitive: "敏感",
};

function formatUnixSeconds(ts: number): string {
  if (!Number.isFinite(ts)) return "-";
  return new Date(ts * 1000).toLocaleString();
}

function locatorText(ref: ReferenceWithHealth["ref"]): string {
  const loc = ref.locator;
  if (loc.kind === "path") return loc.path;
  if (loc.kind === "repo") return loc.local;
  return `${loc.provider}:${loc.objectId}`;
}

interface CollectionDetailPageProps {
  space: Space;
  collection: Collection;
  onBack: () => void;
}

export function CollectionDetailPage({ space, collection, onBack }: CollectionDetailPageProps) {
  const [detail, setDetail] = useState<CollectionDetail | null>(null);
  const [loading, setLoading] = useState(true);
  const [messageApi, messageContextHolder] = message.useMessage();

  const fetchDetail = useCallback(async () => {
    setLoading(true);
    try {
      const result = await collectionGet({ id: collection.id });
      setDetail(result);
    } catch (err) {
      const apiErr = toApiError(err);
      messageApi.error({
        content: apiErr.retryable ? `${apiErr.message}（可重试）` : apiErr.message,
        duration: 3,
      });
    } finally {
      setLoading(false);
    }
  }, [collection.id, messageApi]);

  useEffect(() => {
    fetchDetail();
  }, [fetchDetail]);

  const collapseItems = TYPE_ORDER.map((type) => {
    const items = detail?.referencesByType[type] ?? [];
    const count = items.length;
    return {
      key: type,
      label: (
        <AntSpace>
          <span>{TYPE_LABEL[type]}</span>
          <Badge count={count} showZero color={count > 0 ? "#4a90d9" : "#bfbfbf"} />
          <Text type="secondary" style={{ fontSize: 12 }}>
            {type}
          </Text>
        </AntSpace>
      ),
      children:
        count === 0 ? (
          <Empty
            image={Empty.PRESENTED_IMAGE_SIMPLE}
            description={`暂无${TYPE_LABEL[type]}类引用`}
            style={{ padding: "16px 0" }}
          />
        ) : (
          <List<ReferenceWithHealth>
            dataSource={items}
            renderItem={({ ref, health }) => {
              const meta = HEALTH_META[health];
              return (
                <List.Item key={ref.id}>
                  <List.Item.Meta
                    title={
                      <AntSpace size={8} wrap>
                        <span style={{ fontWeight: 600 }}>{ref.name}</span>
                        <Badge status={meta.color as "success" | "error" | "default"} text={meta.text} />
                        <Tag>{LIFECYCLE_LABEL[ref.lifecycle] ?? ref.lifecycle}</Tag>
                        <Tag color="blue">
                          {CONFIDENTIALITY_LABEL[ref.confidentiality] ?? ref.confidentiality}
                        </Tag>
                      </AntSpace>
                    }
                    description={
                      <AntSpace direction="vertical" size={2} style={{ width: "100%" }}>
                        <Text type="secondary" style={{ fontFamily: "monospace", fontSize: 12 }}>
                          {locatorText(ref)}
                        </Text>
                        <Text type="secondary" style={{ fontSize: 12 }}>
                          创建时间：{formatUnixSeconds(ref.createdAt)}
                        </Text>
                      </AntSpace>
                    }
                  />
                </List.Item>
              );
            }}
          />
        ),
    };
  });

  return (
    <div style={{ maxWidth: 1080 }}>
      {messageContextHolder}

      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          marginBottom: 16,
        }}
      >
        <AntSpace>
          <Button icon={<ArrowLeftOutlined />} onClick={onBack}>
            返回资源集
          </Button>
          <h1 style={{ fontSize: 20, margin: 0 }}>
            {space.name} · {collection.name}
          </h1>
        </AntSpace>
        <Button icon={<ReloadOutlined />} onClick={fetchDetail} loading={loading}>
          刷新
        </Button>
      </div>

      {loading && !detail ? (
        <div style={{ textAlign: "center", padding: "48px 0" }}>
          <Spin size="large" />
        </div>
      ) : detail ? (
        <>
          <Descriptions
            bordered
            size="small"
            column={2}
            style={{ marginBottom: 16 }}
            items={[
              {
                key: "name",
                label: "名称",
                children: <span style={{ fontWeight: 600 }}>{detail.name}</span>,
              },
              {
                key: "status",
                label: "状态",
                children:
                  detail.status === "active" ? (
                    <Tag color="success">活跃</Tag>
                  ) : (
                    <Tag color="warning">已归档</Tag>
                  ),
              },
              {
                key: "summary",
                label: "简介",
                span: 2,
                children: detail.summary ? (
                  <Paragraph style={{ margin: 0 }}>{detail.summary}</Paragraph>
                ) : (
                  <Text type="secondary">—</Text>
                ),
              },
              {
                key: "tags",
                label: "标签",
                span: 2,
                children:
                  detail.tags && detail.tags.length > 0 ? (
                    <AntSpace size={4} wrap>
                      {detail.tags.map((t) => (
                        <Tag key={t}>{t}</Tag>
                      ))}
                    </AntSpace>
                  ) : (
                    <Text type="secondary">—</Text>
                  ),
              },
              {
                key: "createdAt",
                label: "创建时间",
                children: formatUnixSeconds(detail.createdAt),
              },
              {
                key: "updatedAt",
                label: "更新时间",
                children: formatUnixSeconds(detail.updatedAt),
              },
            ]}
          />

          <Collapse
            items={collapseItems}
            defaultActiveKey={TYPE_ORDER.filter(
              (t) => (detail.referencesByType[t] ?? []).length > 0,
            )}
          />
        </>
      ) : (
        <Empty description="未加载到资源集详情" />
      )}
    </div>
  );
}
