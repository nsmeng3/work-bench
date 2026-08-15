import { useState, useEffect, useCallback } from "react";
import { Button, Modal, Segmented, Space as AntSpace, Table, Tag, Tooltip, message } from "antd";
import type { ColumnsType } from "antd/es/table";
import { PlusOutlined } from "@ant-design/icons";
import type { Space } from "../api";
import { spaceList, spaceArchive, spaceRestore, toApiError } from "../api";
import { SpaceDialog } from "../components/SpaceDialog";

type StatusFilter = "active" | "archived";

function formatUnixSeconds(ts: number): string {
  if (!Number.isFinite(ts)) return "-";
  return new Date(ts * 1000).toLocaleString();
}

export function SpacePage() {
  const [spaces, setSpaces] = useState<Space[]>([]);
  const [loading, setLoading] = useState(true);
  const [statusFilter, setStatusFilter] = useState<StatusFilter>("active");

  const [dialogOpen, setDialogOpen] = useState(false);
  const [editingSpace, setEditingSpace] = useState<Space | undefined>(undefined);

  const [messageApi, messageContextHolder] = message.useMessage();
  const [modalApi, modalContextHolder] = Modal.useModal();

  const fetchSpaces = useCallback(async () => {
    setLoading(true);
    try {
      const result = await spaceList({ status: statusFilter });
      setSpaces(result);
    } catch (err) {
      const apiErr = toApiError(err);
      messageApi.error({
        content: apiErr.message,
        duration: 3,
      });
    } finally {
      setLoading(false);
    }
  }, [statusFilter, messageApi]);

  useEffect(() => {
    fetchSpaces();
  }, [fetchSpaces]);

  function handleCreate() {
    setEditingSpace(undefined);
    setDialogOpen(true);
  }

  function handleEdit(space: Space) {
    setEditingSpace(space);
    setDialogOpen(true);
  }

  function handleArchiveClick(space: Space) {
    modalApi.confirm({
      title: "归档空间",
      content: `确定要归档空间"${space.name}"吗？归档后不可编辑，但可随时恢复。`,
      okText: "归档",
      okButtonProps: { danger: true },
      cancelText: "取消",
      async onOk() {
        try {
          await spaceArchive({ id: space.id });
          messageApi.success("已归档");
          fetchSpaces();
        } catch (err) {
          const apiErr = toApiError(err);
          messageApi.error(apiErr.message);
          throw err;
        }
      },
    });
  }

  async function handleRestore(space: Space) {
    try {
      await spaceRestore({ id: space.id });
      messageApi.success("已恢复");
      fetchSpaces();
    } catch (err) {
      const apiErr = toApiError(err);
      messageApi.error(apiErr.message);
    }
  }

  const columns: ColumnsType<Space> = [
    {
      title: "名称",
      dataIndex: "name",
      key: "name",
      render: (_, record) => (
        <AntSpace>
          <span
            style={{
              display: "inline-block",
              width: 12,
              height: 12,
              borderRadius: "50%",
              backgroundColor: record.color || "#4A90D9",
            }}
          />
          <span style={{ fontWeight: 600 }}>{record.name}</span>
        </AntSpace>
      ),
    },
    {
      title: "描述",
      dataIndex: "description",
      key: "description",
      render: (text?: string) => text || <span style={{ color: "#999" }}>—</span>,
    },
    {
      title: "状态",
      dataIndex: "status",
      key: "status",
      width: 100,
      render: (status: Space["status"]) =>
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
      width: 160,
      render: (_, record) => (
        <AntSpace>
          {record.status === "active" ? (
            <>
              <Tooltip title="编辑空间">
                <Button size="small" onClick={() => handleEdit(record)}>
                  编辑
                </Button>
              </Tooltip>
              <Tooltip title="归档后不可编辑">
                <Button size="small" danger onClick={() => handleArchiveClick(record)}>
                  归档
                </Button>
              </Tooltip>
            </>
          ) : (
            <Button size="small" type="primary" ghost onClick={() => handleRestore(record)}>
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
        <h1 style={{ fontSize: 20, margin: 0 }}>空间</h1>
        <AntSpace>
          <Segmented
            value={statusFilter}
            onChange={(v) => setStatusFilter(v as StatusFilter)}
            options={[
              { label: "活跃", value: "active" },
              { label: "已归档", value: "archived" },
            ]}
          />
          <Button type="primary" icon={<PlusOutlined />} onClick={handleCreate}>
            创建空间
          </Button>
        </AntSpace>
      </div>

      <Table<Space>
        rowKey="id"
        loading={loading}
        dataSource={spaces}
        columns={columns}
        pagination={false}
        locale={{
          emptyText:
            statusFilter === "active" ? "暂无活跃空间，点击上方按钮创建。" : "暂无已归档空间。",
        }}
      />

      <SpaceDialog
        open={dialogOpen}
        space={editingSpace}
        onClose={() => setDialogOpen(false)}
        onSaved={fetchSpaces}
      />
    </div>
  );
}
