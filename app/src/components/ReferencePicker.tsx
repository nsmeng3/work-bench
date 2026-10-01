import { useEffect, useMemo, useState } from "react";
import { Cascader, Empty, Input, List, Spin, Tag, Typography, Space as AntSpace } from "antd";
import { PaperClipOutlined, SearchOutlined } from "@ant-design/icons";
import type { Collection, Reference, Space } from "../api";
import { collectionGet, collectionList, spaceList, toApiError } from "../api";

const { Text } = Typography;

/**
 * 资源选择器（M7-2-fix · 挂资源交互修复）
 *
 * 用于 TodoPage 详情 drawer 挂资源，替代手输 refId。
 *
 * 两种使用方式：
 * 1. 级联浏览：空间 → 资源集 → 资源
 * 2. 顶部搜索框：按名称模糊匹配（跨空间）
 *
 * 点击即选（触发 onSelect），已挂载的 refId 会在列表项右侧打 "已挂" 标。
 */

export interface ReferencePickerProps {
  /** 已挂载的 refId 列表（用于置灰/标记） */
  attachedRefIds: string[];
  /** 用户点击某个资源 */
  onSelect: (ref: Reference) => void;
  /** 可选：限定在某个空间内挑（空间详情页"待办"tab 用） */
  lockedSpaceId?: string;
}

interface CascaderOption {
  value: string;
  label: string;
  children?: CascaderOption[];
  /** 仅叶子节点有 */
  ref?: Reference;
}

export function ReferencePicker({ attachedRefIds, onSelect, lockedSpaceId }: ReferencePickerProps) {
  const [spaces, setSpaces] = useState<Space[]>([]);
  const [collectionsBySpace, setCollectionsBySpace] = useState<Record<string, Collection[]>>({});
  const [refsByCollection, setRefsByCollection] = useState<Record<string, Reference[]>>({});
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [searchText, setSearchText] = useState("");
  const [cascaderValue, setCascaderValue] = useState<string[] | undefined>(undefined);

  /* ---------------- 数据加载 ---------------- */

  useEffect(() => {
    let cancelled = false;
    (async () => {
      setLoading(true);
      setError(null);
      try {
        const spaceRows = await spaceList({});
        if (cancelled) return;
        setSpaces(spaceRows);

        const targetSpaces = lockedSpaceId
          ? spaceRows.filter((s) => s.id === lockedSpaceId)
          : spaceRows;

        const collMap: Record<string, Collection[]> = {};
        const refMap: Record<string, Reference[]> = {};
        for (const sp of targetSpaces) {
          const colls = await collectionList({ spaceId: sp.id });
          if (cancelled) return;
          collMap[sp.id] = colls;
          for (const coll of colls) {
            const detail = await collectionGet({ id: coll.id });
            if (cancelled) return;
            // 扁平化 referencesByType
            const allRefs: Reference[] = Object.values(detail.referencesByType)
              .flat()
              .map((rw) => rw.ref);
            refMap[coll.id] = allRefs;
          }
        }
        if (cancelled) return;
        setCollectionsBySpace(collMap);
        setRefsByCollection(refMap);
      } catch (err) {
        if (cancelled) return;
        const apiErr = toApiError(err);
        setError(apiErr.message);
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [lockedSpaceId]);

  /* ---------------- 搜索结果 ---------------- */

  const allRefs = useMemo(() => {
    return Object.values(refsByCollection).flat();
  }, [refsByCollection]);

  const searchResults = useMemo(() => {
    const kw = searchText.trim().toLowerCase();
    if (!kw) return null;
    return allRefs.filter((r) => r.name.toLowerCase().includes(kw));
  }, [allRefs, searchText]);

  /* ---------------- Cascader 选项 ---------------- */

  const cascaderOptions = useMemo<CascaderOption[]>(() => {
    const targetSpaces = lockedSpaceId
      ? spaces.filter((s) => s.id === lockedSpaceId)
      : spaces;
    return targetSpaces.map((sp) => ({
      value: sp.id,
      label: sp.name,
      children: (collectionsBySpace[sp.id] ?? []).map((coll) => ({
        value: coll.id,
        label: coll.name,
        children: (refsByCollection[coll.id] ?? []).map((ref) => ({
          value: ref.id,
          label: ref.name,
          ref,
        })),
      })),
    }));
  }, [spaces, collectionsBySpace, refsByCollection, lockedSpaceId]);

  /* ---------------- 渲染 ---------------- */

  function renderRefItem(ref: Reference) {
    const attached = attachedRefIds.includes(ref.id);
    return (
      <List.Item
        key={ref.id}
        style={{
          cursor: attached ? "not-allowed" : "pointer",
          opacity: attached ? 0.5 : 1,
          padding: "6px 8px",
        }}
        onClick={() => {
          if (!attached) onSelect(ref);
        }}
      >
        <AntSpace size={8}>
          <PaperClipOutlined />
          <span>{ref.name}</span>
          <Text type="secondary" style={{ fontSize: 12 }}>
            {ref.type}
          </Text>
          {attached && <Tag color="green">已挂</Tag>}
        </AntSpace>
      </List.Item>
    );
  }

  if (loading) {
    return (
      <div style={{ textAlign: "center", padding: "24px 0" }}>
        <Spin size="small" tip="加载空间/资源集/资源…" />
      </div>
    );
  }
  if (error) {
    return <Text type="danger">加载失败：{error}</Text>;
  }

  return (
    <div>
      <Input
        prefix={<SearchOutlined />}
        placeholder="搜索资源名称（跨空间）…"
        value={searchText}
        onChange={(e) => setSearchText(e.target.value)}
        allowClear
        style={{ marginBottom: 8 }}
      />
      {searchResults !== null ? (
        searchResults.length === 0 ? (
          <Empty description="无匹配资源" image={Empty.PRESENTED_IMAGE_SIMPLE} />
        ) : (
          <List
            size="small"
            dataSource={searchResults}
            renderItem={renderRefItem}
            style={{ maxHeight: 260, overflow: "auto", border: "1px solid #f0f0f0", borderRadius: 4 }}
          />
        )
      ) : (
        <Cascader
          options={cascaderOptions}
          value={cascaderValue}
          onChange={(val) => {
            setCascaderValue(val as string[]);
            // 末级选中即触发 onSelect
            if (val && val.length === 3) {
              const refId = val[2] as string;
              const ref = allRefs.find((r) => r.id === refId);
              if (ref && !attachedRefIds.includes(ref.id)) {
                onSelect(ref);
                // 选中后清空，允许连续点选
                setCascaderValue(undefined);
              }
            }
          }}
          placeholder="空间 / 资源集 / 资源"
          style={{ width: "100%" }}
          expandTrigger="hover"
          showSearch={{
            filter: (input, path) =>
              path.some((opt) => opt.label.toLowerCase().includes(input.toLowerCase())),
          }}
        />
      )}
    </div>
  );
}
