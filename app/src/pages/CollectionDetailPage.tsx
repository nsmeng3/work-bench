import { useState, useEffect, useCallback } from "react";
import {
  Badge,
  Button,
  Collapse,
  Descriptions,
  Empty,
  Form,
  Input,
  List,
  Modal,
  Select,
  Space as AntSpace,
  Spin,
  Switch,
  Tag,
  Typography,
  message,
} from "antd";
import { ArrowLeftOutlined, EditOutlined, PlusOutlined, ReloadOutlined } from "@ant-design/icons";
import type {
  Collection,
  CollectionDetail,
  Reference,
  ReferenceConfidentiality,
  ReferenceHealth,
  ReferenceLifecycle,
  ReferenceType,
  ReferenceWithHealth,
  Space,
} from "../api";
import { collectionGet, refUpdate, toApiError } from "../api";
import { ReferenceCreateDialog } from "../components/ReferenceCreateDialog";

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

const LIFECYCLE_LABEL: Record<ReferenceLifecycle, string> = {
  active: "活跃",
  staged: "已暂存",
  delivered: "已交付",
  archived: "已归档",
};

const CONFIDENTIALITY_LABEL: Record<ReferenceConfidentiality, string> = {
  public: "公开",
  internal: "内部",
  customer_restricted: "客户受限",
  sensitive: "敏感",
};

const LIFECYCLE_OPTIONS: { value: ReferenceLifecycle; label: string }[] = (
  Object.keys(LIFECYCLE_LABEL) as ReferenceLifecycle[]
).map((v) => ({ value: v, label: LIFECYCLE_LABEL[v] }));

const CONFIDENTIALITY_OPTIONS: { value: ReferenceConfidentiality; label: string }[] = (
  Object.keys(CONFIDENTIALITY_LABEL) as ReferenceConfidentiality[]
).map((v) => ({ value: v, label: CONFIDENTIALITY_LABEL[v] }));

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

/** 编辑表单字段 — §2.5 ref_update 仅允许管理属性 */
interface RefEditFormValues {
  name: string;
  description?: string;
  tags?: string[];
  lifecycle: ReferenceLifecycle;
  confidentiality: ReferenceConfidentiality;
  indexed: boolean;
}

export function CollectionDetailPage({ space, collection, onBack }: CollectionDetailPageProps) {
  const [detail, setDetail] = useState<CollectionDetail | null>(null);
  const [loading, setLoading] = useState(true);
  const [createOpen, setCreateOpen] = useState(false);
  const [editingRef, setEditingRef] = useState<Reference | null>(null);
  const [saving, setSaving] = useState(false);
  const [form] = Form.useForm<RefEditFormValues>();
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

  const openEditModal = (ref: Reference) => {
    setEditingRef(ref);
    form.setFieldsValue({
      name: ref.name,
      description: ref.description,
      tags: ref.tags ?? [],
      lifecycle: ref.lifecycle,
      confidentiality: ref.confidentiality,
      indexed: ref.indexed,
    });
  };

  const closeEditModal = () => {
    if (saving) return;
    setEditingRef(null);
    form.resetFields();
  };

  const handleSave = async () => {
    if (!editingRef) return;
    let values: RefEditFormValues;
    try {
      values = await form.validateFields();
    } catch {
      return; // 校验失败，antd 已提示
    }
    setSaving(true);
    try {
      await refUpdate({
        id: editingRef.id,
        name: values.name.trim(),
        description: values.description?.trim() || undefined,
        tags: values.tags ?? [],
        lifecycle: values.lifecycle,
        confidentiality: values.confidentiality,
        indexed: values.indexed,
      });
      messageApi.success({ content: "引用已更新", duration: 2 });
      setEditingRef(null);
      form.resetFields();
      await fetchDetail();
    } catch (err) {
      const apiErr = toApiError(err);
      messageApi.error({
        content: apiErr.retryable ? `${apiErr.message}（可重试）` : apiErr.message,
        duration: 3,
      });
    } finally {
      setSaving(false);
    }
  };

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
                <List.Item
                  key={ref.id}
                  actions={[
                    <Button
                      key="edit"
                      type="text"
                      size="small"
                      icon={<EditOutlined />}
                      onClick={() => openEditModal(ref)}
                    >
                      编辑
                    </Button>,
                  ]}
                >
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
        <AntSpace>
          <Button
            type="primary"
            icon={<PlusOutlined />}
            onClick={() => setCreateOpen(true)}
            disabled={detail?.status === "archived"}
          >
            添加引用
          </Button>
          <Button icon={<ReloadOutlined />} onClick={fetchDetail} loading={loading}>
            刷新
          </Button>
        </AntSpace>
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

      <ReferenceCreateDialog
        open={createOpen}
        collectionId={collection.id}
        onClose={() => setCreateOpen(false)}
        onCreated={fetchDetail}
      />
      <Modal
        title="编辑引用"
        open={editingRef !== null}
        onOk={handleSave}
        onCancel={closeEditModal}
        confirmLoading={saving}
        okText="保存"
        cancelText="取消"
        forceRender
        maskClosable={false}
      >
        {editingRef && (
          <div style={{ marginBottom: 12 }}>
            <Text type="secondary" style={{ fontSize: 12 }}>
              类型与定位不可修改：
              {TYPE_LABEL[editingRef.type]} · {locatorText(editingRef)}
            </Text>
          </div>
        )}
        <Form form={form} layout="vertical" preserve={false}>
          <Form.Item
            name="name"
            label="名称"
            rules={[
              { required: true, message: "请输入名称" },
              { whitespace: true, message: "名称不能为空" },
            ]}
          >
            <Input placeholder="引用名称" maxLength={120} />
          </Form.Item>
          <Form.Item name="description" label="描述">
            <Input.TextArea rows={3} placeholder="可选描述" maxLength={500} />
          </Form.Item>
          <Form.Item name="tags" label="标签">
            <Select mode="tags" placeholder="输入后回车添加标签" tokenSeparators={[","]} />
          </Form.Item>
          <Form.Item name="lifecycle" label="生命周期" rules={[{ required: true }]}>
            <Select options={LIFECYCLE_OPTIONS} />
          </Form.Item>
          <Form.Item name="confidentiality" label="保密级别" rules={[{ required: true }]}>
            <Select options={CONFIDENTIALITY_OPTIONS} />
          </Form.Item>
          <Form.Item name="indexed" label="纳入索引" valuePropName="checked">
            <Switch />
          </Form.Item>
        </Form>
      </Modal>
    </div>
  );
}
