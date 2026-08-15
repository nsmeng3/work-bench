import { useState, useEffect, useCallback, useMemo } from "react";
import {
  Checkbox,
  Empty,
  Input,
  Pagination,
  Space as AntSpace,
  Spin,
  Table,
  Tag,
  Typography,
  message,
} from "antd";
import type { ColumnsType } from "antd/es/table";
import type {
  FacetValue,
  QueryFacetsOutput,
  QueryRefsInput,
  Reference,
  ReferenceConfidentiality,
  ReferenceLifecycle,
  ReferenceType,
} from "../api";
import { queryFacets, queryRefs, toApiError } from "../api";

/**
 * 筛选页 — 任务包 m2-2.12。
 * 契约：详细设计说明书 §2.9 query_refs / query_facets。
 *
 * 取舍说明（任务要求 5）：
 * - 侧边栏计数采用「全量计数」：仅在挂载时调用一次 query_facets，
 *   不随筛选条件变化刷新。原因：契约 §2.9 query_facets 入参仅支持 spaceId，
 *   不接受其他筛选维度，后端本就返回全量计数；若前端再按当前筛选结果
 *   本地聚合，会与后端语义不一致。
 */

const PAGE_SIZE = 50; // 契约默认 limit

const TYPE_LABELS: Record<ReferenceType, string> = {
  code: "代码",
  document: "文档",
  data: "数据",
  artifact: "制品",
  tool: "工具",
  media: "媒体",
};

const LIFECYCLE_LABELS: Record<ReferenceLifecycle, string> = {
  active: "活跃",
  staged: "暂存",
  delivered: "已交付",
  archived: "已归档",
};

const CONFIDENTIALITY_LABELS: Record<ReferenceConfidentiality, string> = {
  public: "公开",
  internal: "内部",
  customer_restricted: "客户受限",
  sensitive: "敏感",
};

const LIFECYCLE_COLORS: Record<ReferenceLifecycle, string> = {
  active: "success",
  staged: "processing",
  delivered: "blue",
  archived: "default",
};

const CONFIDENTIALITY_COLORS: Record<ReferenceConfidentiality, string> = {
  public: "green",
  internal: "blue",
  customer_restricted: "orange",
  sensitive: "red",
};

function formatUnixSeconds(ts: number): string {
  if (!Number.isFinite(ts)) return "-";
  return new Date(ts * 1000).toLocaleString();
}

interface FacetGroupProps {
  title: string;
  values: FacetValue[];
  selected: string[];
  onChange: (next: string[]) => void;
  /** 单选模式：保留最后一次勾选（用于 type/lifecycle/confidentiality） */
  single?: boolean;
  labelMap?: Record<string, string>;
}

function FacetGroup({ title, values, selected, onChange, single, labelMap }: FacetGroupProps) {
  return (
    <div style={{ marginBottom: 16 }}>
      <Typography.Text strong style={{ display: "block", marginBottom: 8 }}>
        {title}
      </Typography.Text>
      {values.length === 0 ? (
        <Typography.Text type="secondary" style={{ fontSize: 12 }}>
          暂无可选值
        </Typography.Text>
      ) : (
        <Checkbox.Group
          style={{ display: "flex", flexDirection: "column", gap: 4 }}
          value={selected}
          onChange={(vals) => {
            const next = vals.map(String);
            if (single) {
              // 单选：找出新增的那一项；若全部取消则为空
              const added = next.find((v) => !selected.includes(v));
              onChange(added ? [added] : []);
            } else {
              onChange(next);
            }
          }}
          options={values.map((v) => ({
            value: v.value,
            label: (
              <span>
                {labelMap?.[v.value] ?? v.value}
                <Typography.Text type="secondary" style={{ marginLeft: 6, fontSize: 12 }}>
                  ({v.count})
                </Typography.Text>
              </span>
            ),
          }))}
        />
      )}
    </div>
  );
}

export function FilterPage() {
  const [facets, setFacets] = useState<QueryFacetsOutput | null>(null);
  const [facetsLoading, setFacetsLoading] = useState(true);

  // 筛选状态（受控）
  const [typeSel, setTypeSel] = useState<string[]>([]);
  const [lifecycleSel, setLifecycleSel] = useState<string[]>([]);
  const [confSel, setConfSel] = useState<string[]>([]);
  const [tagsSel, setTagsSel] = useState<string[]>([]);
  const [keyword, setKeyword] = useState("");
  const [page, setPage] = useState(1);

  const [items, setItems] = useState<Reference[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(false);

  const [messageApi, messageContextHolder] = message.useMessage();

  // 挂载时拉一次 facets（全量计数）
  useEffect(() => {
    let cancelled = false;
    (async () => {
      setFacetsLoading(true);
      try {
        const f = await queryFacets({});
        if (!cancelled) setFacets(f);
      } catch (err) {
        if (!cancelled) {
          const apiErr = toApiError(err);
          messageApi.error(apiErr.message);
        }
      } finally {
        if (!cancelled) setFacetsLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [messageApi]);

  const fetchRefs = useCallback(async () => {
    setLoading(true);
    try {
      const input: QueryRefsInput = {
        limit: PAGE_SIZE,
        offset: (page - 1) * PAGE_SIZE,
      };
      if (typeSel[0]) input.type = typeSel[0] as ReferenceType;
      if (lifecycleSel[0]) input.lifecycle = lifecycleSel[0] as ReferenceLifecycle;
      if (confSel[0]) input.confidentiality = confSel[0] as ReferenceConfidentiality;
      if (tagsSel.length > 0) input.tags = tagsSel;
      if (keyword.trim()) input.keyword = keyword.trim();
      const out = await queryRefs(input);
      setItems(out.items);
      setTotal(out.total);
    } catch (err) {
      const apiErr = toApiError(err);
      messageApi.error(apiErr.message);
    } finally {
      setLoading(false);
    }
  }, [typeSel, lifecycleSel, confSel, tagsSel, keyword, page, messageApi]);

  useEffect(() => {
    fetchRefs();
  }, [fetchRefs]);

  // 筛选条件变化时回到第一页（page 自身变化除外）
  useEffect(() => {
    setPage(1);
  }, [typeSel, lifecycleSel, confSel, tagsSel, keyword]);

  const columns: ColumnsType<Reference> = useMemo(
    () => [
      {
        title: "名称",
        dataIndex: "name",
        key: "name",
        render: (_, r) => (
          <div>
            <div style={{ fontWeight: 600 }}>{r.name}</div>
            {r.description && (
              <Typography.Text type="secondary" style={{ fontSize: 12 }}>
                {r.description}
              </Typography.Text>
            )}
          </div>
        ),
      },
      {
        title: "类型",
        dataIndex: "type",
        key: "type",
        width: 90,
        render: (t: ReferenceType) => <Tag>{TYPE_LABELS[t] ?? t}</Tag>,
      },
      {
        title: "所属资源集",
        dataIndex: "collectionId",
        key: "collectionId",
        width: 160,
        render: (id: string) => <Typography.Text code>{id}</Typography.Text>,
      },
      {
        title: "生命周期",
        dataIndex: "lifecycle",
        key: "lifecycle",
        width: 110,
        render: (v: ReferenceLifecycle) => (
          <Tag color={LIFECYCLE_COLORS[v]}>{LIFECYCLE_LABELS[v] ?? v}</Tag>
        ),
      },
      {
        title: "保密级别",
        dataIndex: "confidentiality",
        key: "confidentiality",
        width: 110,
        render: (v: ReferenceConfidentiality) => (
          <Tag color={CONFIDENTIALITY_COLORS[v]}>{CONFIDENTIALITY_LABELS[v] ?? v}</Tag>
        ),
      },
      {
        title: "标签",
        dataIndex: "tags",
        key: "tags",
        render: (tags?: string[]) =>
          tags && tags.length > 0 ? (
            <AntSpace size={4} wrap>
              {tags.map((t) => (
                <Tag key={t}>{t}</Tag>
              ))}
            </AntSpace>
          ) : (
            <span style={{ color: "#999" }}>—</span>
          ),
      },
      {
        title: "创建时间",
        dataIndex: "createdAt",
        key: "createdAt",
        width: 170,
        render: (ts: number) => formatUnixSeconds(ts),
      },
    ],
    [],
  );

  return (
    <div style={{ display: "flex", gap: 24, alignItems: "flex-start" }}>
      {messageContextHolder}

      {/* 左侧筛选侧边栏 */}
      <div
        style={{
          width: 240,
          flexShrink: 0,
          padding: 16,
          background: "#fff",
          border: "1px solid #f0f0f0",
          borderRadius: 8,
        }}
      >
        <Typography.Title level={5} style={{ marginTop: 0 }}>
          筛选
        </Typography.Title>
        {facetsLoading ? (
          <Spin size="small" />
        ) : facets ? (
          <>
            <FacetGroup
              title="类型"
              values={facets.types}
              selected={typeSel}
              onChange={setTypeSel}
              single
              labelMap={TYPE_LABELS}
            />
            <FacetGroup
              title="生命周期"
              values={facets.lifecycles}
              selected={lifecycleSel}
              onChange={setLifecycleSel}
              single
              labelMap={LIFECYCLE_LABELS}
            />
            <FacetGroup
              title="保密级别"
              values={facets.confidentialities}
              selected={confSel}
              onChange={setConfSel}
              single
              labelMap={CONFIDENTIALITY_LABELS}
            />
            <FacetGroup
              title="标签（多选 AND）"
              values={facets.tags}
              selected={tagsSel}
              onChange={setTagsSel}
            />
          </>
        ) : (
          <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description="facets 加载失败" />
        )}
      </div>

      {/* 右侧结果区 */}
      <div style={{ flex: 1, minWidth: 0 }}>
        <div
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            marginBottom: 12,
            gap: 16,
          }}
        >
          <AntSpace>
            <Typography.Title level={4} style={{ margin: 0 }}>
              引用列表
            </Typography.Title>
            <Typography.Text type="secondary">共 {total} 条</Typography.Text>
          </AntSpace>
          <Input.Search
            allowClear
            placeholder="按名称 / 描述 / 标签搜索"
            style={{ width: 320 }}
            onSearch={(v) => setKeyword(v)}
            onChange={(e) => {
              // 清空时立即触发
              if (!e.target.value) setKeyword("");
            }}
          />
        </div>

        <Table<Reference>
          rowKey="id"
          loading={loading}
          dataSource={items}
          columns={columns}
          pagination={false}
          locale={{ emptyText: "无匹配引用，调整筛选条件试试。" }}
        />

        <div style={{ display: "flex", justifyContent: "flex-end", marginTop: 16 }}>
          <Pagination
            current={page}
            pageSize={PAGE_SIZE}
            total={total}
            showSizeChanger={false}
            onChange={(p) => setPage(p)}
          />
        </div>
      </div>
    </div>
  );
}
