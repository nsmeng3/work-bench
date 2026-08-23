import { useCallback, useEffect, useState } from "react";
import {
  Button,
  Card,
  Empty,
  Form,
  Input,
  List,
  Modal,
  Space,
  Typography,
  message,
} from "antd";
import { DeleteOutlined, FolderAddOutlined } from "@ant-design/icons";
import type { WatchDirConfig } from "../api";
import {
  inboxGetWatchDirs,
  inboxSetWatchDir,
  inboxUnsetWatchDir,
  toApiError,
} from "../api";
import { RootDirPicker } from "../components/RootDirPicker";

/**
 * 设置页 — 监控目录配置面板（M6-6.3）。
 * 契约：详细设计说明书 §2.8。
 *
 * 功能：
 * - 展示已添加的监控目录列表（按 path 升序）
 * - 添加新目录（表单校验：path 必填、绝对路径）
 * - 移除目录（确认对话框）
 */

export function SettingsPage() {
  const [dirs, setDirs] = useState<WatchDirConfig[]>([]);
  const [loading, setLoading] = useState(false);
  const [addModalOpen, setAddModalOpen] = useState(false);
  const [removeModalOpen, setRemoveModalOpen] = useState(false);
  const [removingDir, setRemovingDir] = useState<WatchDirConfig | null>(null);
  const [form] = Form.useForm();
  const [submitting, setSubmitting] = useState(false);

  const loadDirs = useCallback(async () => {
    setLoading(true);
    try {
      const list = await inboxGetWatchDirs();
      setDirs(list.sort((a, b) => a.path.localeCompare(b.path)));
    } catch (err) {
      const apiErr = toApiError(err);
      message.error(`加载监控目录失败：${apiErr.message}`);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadDirs();
  }, [loadDirs]);

  async function handleAddSubmit() {
    try {
      const values = await form.validateFields();
      setSubmitting(true);
      await inboxSetWatchDir({
        id: crypto.randomUUID(),
        path: values.path,
        name: values.name || values.path.split("/").pop() || "未命名",
        description: values.description,
      });
      message.success("监控目录添加成功");
      setAddModalOpen(false);
      void loadDirs();
    } catch (err) {
      if (err && typeof err === "object" && "errorFields" in err) {
        return;
      }
      const apiErr = toApiError(err);
      message.error(`添加失败：${apiErr.message}`);
    } finally {
      setSubmitting(false);
    }
  }

  function handleRemove(dir: WatchDirConfig) {
    setRemovingDir(dir);
    setRemoveModalOpen(true);
  }

  async function handleRemoveConfirm() {
    if (!removingDir) return;
    setSubmitting(true);
    try {
      await inboxUnsetWatchDir({ path: removingDir.path });
      message.success("监控目录已移除");
      setRemoveModalOpen(false);
      setRemovingDir(null);
      void loadDirs();
    } catch (err) {
      const apiErr = toApiError(err);
      message.error(`移除失败：${apiErr.message}`);
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <div style={{ maxWidth: 800, margin: "0 auto" }}>
      <Typography.Title level={3}>设置</Typography.Title>

      <Card
        title="监控目录"
        extra={
          <Button
            type="primary"
            icon={<FolderAddOutlined />}
            onClick={() => setAddModalOpen(true)}
          >
            添加目录
          </Button>
        }
      >
        <Typography.Paragraph type="secondary" style={{ marginBottom: 16 }}>
          监控目录中的新文件会自动进入收件箱。请使用绝对路径。
        </Typography.Paragraph>

        <List<WatchDirConfig>
          loading={loading}
          dataSource={dirs}
          rowKey="path"
          locale={{ emptyText: <Empty description="暂无监控目录" /> }}
          renderItem={(dir) => (
            <List.Item
              actions={[
                <Button
                  key="remove"
                  type="text"
                  danger
                  icon={<DeleteOutlined />}
                  onClick={() => handleRemove(dir)}
                >
                  移除
                </Button>,
              ]}
            >
              <List.Item.Meta
                title={dir.name || dir.path}
                description={
                  <Space direction="vertical" size={0}>
                    <Typography.Text type="secondary" style={{ fontSize: 12 }}>
                      {dir.path}
                    </Typography.Text>
                    {dir.description && (
                      <Typography.Text type="secondary" style={{ fontSize: 12 }}>
                        {dir.description}
                      </Typography.Text>
                    )}
                  </Space>
                }
              />
            </List.Item>
          )}
        />
      </Card>

      {/* 添加目录对话框 */}
      <Modal
        title="添加监控目录"
        open={addModalOpen}
        onOk={handleAddSubmit}
        onCancel={() => setAddModalOpen(false)}
        confirmLoading={submitting}
        okText="添加"
        cancelText="取消"
        destroyOnHidden
      >
        <Form form={form} layout="vertical">
          <Form.Item
            name="path"
            label="目录路径"
            rules={[
              { required: true, message: "请输入目录路径" },
              {
                pattern: /^\/.*/,
                message: "请输入绝对路径（以 / 开头）",
              },
            ]}
          >
            <RootDirPicker
              value={form.getFieldValue("path") || ""}
              onChange={(path) => form.setFieldValue("path", path)}
              placeholder="例如 /Users/you/Documents"
            />
          </Form.Item>
          <Form.Item name="name" label="名称（可选）">
            <Input placeholder="默认为目录名" />
          </Form.Item>
          <Form.Item name="description" label="描述（可选）">
            <Input.TextArea rows={2} placeholder="监控目录的用途说明" />
          </Form.Item>
        </Form>
      </Modal>

      {/* 移除确认对话框 */}
      <Modal
        title="移除监控目录"
        open={removeModalOpen}
        onOk={handleRemoveConfirm}
        onCancel={() => {
          setRemoveModalOpen(false);
          setRemovingDir(null);
        }}
        confirmLoading={submitting}
        okText="移除"
        okButtonProps={{ danger: true }}
        cancelText="取消"
      >
        <Typography.Paragraph>
          确定要移除监控目录「{removingDir?.name || removingDir?.path}」吗？
        </Typography.Paragraph>
        <Typography.Paragraph type="secondary" style={{ fontSize: 12 }}>
          移除后，该目录中的新文件将不再进入收件箱。已存在的收件箱条目不受影响。
        </Typography.Paragraph>
      </Modal>
    </div>
  );
}
