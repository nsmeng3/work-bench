import { useState, useEffect, useCallback } from "react";
import { Button, Modal, Segmented, Space as AntSpace, Table, Tag, Tooltip, message } from "antd";
import type { ColumnsType } from "antd/es/table";
import { ArrowLeftOutlined, PlusOutlined } from "@ant-design/icons";
import type { Collection, Space } from "../api";
import { collectionList, collectionArchive, collectionRestore, toApiError } from "../api";
import { CollectionDialog } from "../components/CollectionDialog";

type StatusFilter = "active" | "archived";

function formatUnixSeconds(ts: number): string {
  if (!Number.isFinite(ts)) return "-";
  return new Date(ts * 1000).toLocaleString();
}

interface CollectionPageProps {
  space: Space;
  onBack: () => void;
  /** 进入某资源集的详情页 */
  onEnterCollection: (collection: Collection) => void;
}

export function CollectionPage({ space, onBack, onEnterCollection }: CollectionPageProps) {
  const [collections, setCollections] = useState<Collection[]>([]);
  const [loading, setLoading] = useState(true);
  const [statusFilter, setStatusFilter] = useState<StatusFilter>("active");

  const [dialogOpen, setDialogOpen] = useState(false);
  const [editingCollection, setEditingCollection] = useState<Collection | undefined>(undefined);

  const [messageApi, messageContextHolder] = message.useMessage();
  const [modalApi, modalContextHolder] = Modal.useModal();

  const fetchCollections = useCallback(async () => {
    setLoading(true);
    try {
      const result = await collectionList({ spaceId: space.id, status: statusFilter });
      setCollections(result);
    } catch (err) {
      const apiErr = toApiError(err);
      messageApi.error({
        content: apiErr.message,
        duration: 3,
      });
    } finally {
      setLoading(false);
    }
  }, [space.id, statusFilter, messageApi]);

  useEffect(() => {
    fetchCollections();
  }, [fetchCollections]);

  function handleCreate() {
    setEditingCollection(undefined);
    setDialogOpen(true);
  }

  function handleEdit(collection: Collection) {
    setEditingCollection(collection);
    setDialogOpen(true);
  }

  function handleArchiveClick(collection: Collection) {
    modalApi.confirm({
      title: "归档资源集",
      content: `确定要归档资源集"${collection.name}"吗？归档后不可编辑，但可随时恢复；不影响其下已关联的资源。`,
      okText: "归档",
      okButtonProps: { danger: true },
      cancelText: "取消",
      async onOk() {
        try {
          await collectionArchive({ id: collection.id });
          messageApi.success("已归档");
          fetchCollections();
        } catch (err) {
          const apiErr = toApiError(err);
          messageApi.error(apiErr.message);
          throw err;
        }
      },
    });
  }

  async function handleRestore(collection: Collection) {
    try {
      await collectionRestore({ id: collection.id });
      messageApi.success("已恢复");
      fetchCollections();
    } catch (err) {
      const apiErr = toApiError(err);
      messageApi.error(apiErr.message);
    }
  }

  const columns: ColumnsType<Collection> = [
    {
      title: "名称",
      dataIndex: "name",
      key: "name",
      render: (_, record) => (
        <a onClick={() => onEnterCollection(record)} style={{ fontWeight: 600 }}>
          {record.name}
        </a>
      ),
    },
    {
      title: "简介",
      dataIndex: "summary",
      key: "summary",
      render: (text?: string) => text || <span style={{ color: "#999" }}>—</span>,
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
      title: "状态",
      dataIndex: "status",
      key: "status",
      width: 100,
      render: (status: Collection["status"]) =>
        status === "active" ? <Tag color="success">活跃</Tag> : <Tag color="warning">已归档</Tag>,
    },
    {
      title: "创建时间",
      dataIndex: "createdAt",
      key: "createdAt",
      width: 180,
      render: (ts: number) => formatUnixSeconds(ts),
    },
    {
      title: "操作",
      key: "actions",
      width: 220,
      render: (_, record) => (
        <AntSpace>
          {record.status === "active" ? (
            <>
              <Tooltip title="编辑资源集">
                <Button
                  size="small"
                  onClick={(e) => {
                    e.stopPropagation();
                    handleEdit(record);
                  }}
                >
                  编辑
                </Button>
              </Tooltip>
              <Tooltip title="归档后不可编辑">
                <Button
                  size="small"
                  danger
                  onClick={(e) => {
                    e.stopPropagation();
                    handleArchiveClick(record);
                  }}
                >
                  归档
                </Button>
              </Tooltip>
            </>
          ) : (
            <Button
              size="small"
              type="primary"
              ghost
              onClick={(e) => {
                e.stopPropagation();
                handleRestore(record);
              }}
            >
              恢复
            </Button>
          )}
        </AntSpace>
      ),
    },
  ];

  return (
    <div style={{ maxWidth: 960 }}>
      {messageContextHolder}
      {modalContextHolder}

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
            返回空间
          </Button>
          <h1 style={{ fontSize: 20, margin: 0 }}>
            {space.name} · 资源集
          </h1>
        </AntSpace>
        <AntSpace>
          <Segmented
            value={statusFilter}
            onChange={(v) => setStatusFilter(v as StatusFilter)}
            options={[
              { label: "活跃", value: "active" },
              { label: "已归档", value: "archived" },
            ]}
          />
          <Button
            type="primary"
            icon={<PlusOutlined />}
            onClick={handleCreate}
            disabled={space.status === "archived"}
          >
            创建资源集
          </Button>
        </AntSpace>
      </div>

      <Table<Collection>
        rowKey="id"
        loading={loading}
        dataSource={collections}
        columns={columns}
        pagination={false}
        onRow={(record) => ({
          onClick: () => onEnterCollection(record),
          style: { cursor: "pointer" },
        })}
        locale={{
          emptyText:
            statusFilter === "active"
              ? "暂无活跃资源集，点击上方按钮创建。"
              : "暂无已归档资源集。",
        }}
      />

      <CollectionDialog
        open={dialogOpen}
        spaceId={space.id}
        collection={editingCollection}
        onClose={() => setDialogOpen(false)}
        onSaved={fetchCollections}
      />
    </div>
  );
}
