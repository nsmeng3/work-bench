import { useCallback, useEffect, useState } from "react";
import {
  Button,
  Card,
  Empty,
  Form,
  Input,
  List,
  Modal,
  Radio,
  Select,
  Space,
  Tag,
  Typography,
  message,
} from "antd";
import {
  DeleteOutlined,
  EditOutlined,
  FolderAddOutlined,
  SettingOutlined,
} from "@ant-design/icons";
import type {
  WatchDirConfig,
  RootDirStatus,
  StorageSourceInfo,
  DefaultAppConfig,
  DefaultHome,
  ReferenceType,
} from "../api";
import {
  inboxGetWatchDirs,
  inboxSetWatchDir,
  inboxUnsetWatchDir,
  settingsGetRootDir,
  settingsListSources,
  settingsUpdateSource,
  settingsGetDefaultApp,
  settingsSetDefaultApp,
  settingsGetDefaultHome,
  settingsSetDefaultHome,
  toApiError,
} from "../api";
import { RootDirPicker } from "../components/RootDirPicker";
import { MigrationWizard } from "../components/MigrationWizard";

/**
 * 设置页 — 四分区卡片式布局（M6-6.5）。
 * 契约：详细设计说明书 §2.8。
 *
 * 分区：
 * 1. 根目录：显示当前根目录 + 类型子目录状态
 * 2. 监控目录：复用 M6-6.3 逻辑
 * 3. 存储源：显示 LocalFsSource 信息 + 能力标签 + 名称编辑
 * 4. 默认程序：按 6 种类型配置默认打开方式
 */

const REFERENCE_TYPES: { key: ReferenceType; label: string }[] = [
  { key: "code", label: "代码" },
  { key: "document", label: "文档" },
  { key: "data", label: "数据" },
  { key: "artifact", label: "产物" },
  { key: "tool", label: "工具" },
  { key: "media", label: "媒体" },
];

const TYPE_SUBDIRS = ["Code", "Documents", "Data", "Artifacts", "Tools", "Media"];

export function SettingsPage() {
  // ---------- 监控目录状态 ----------
  const [dirs, setDirs] = useState<WatchDirConfig[]>([]);
  const [dirsLoading, setDirsLoading] = useState(false);
  const [addModalOpen, setAddModalOpen] = useState(false);
  const [removeModalOpen, setRemoveModalOpen] = useState(false);
  const [removingDir, setRemovingDir] = useState<WatchDirConfig | null>(null);
  const [addForm] = Form.useForm();
  const [submitting, setSubmitting] = useState(false);

  // ---------- 根目录状态 ----------
  const [rootDirStatus, setRootDirStatus] = useState<RootDirStatus | null>(null);
  const [rootDirLoading, setRootDirLoading] = useState(false);
  const [migrationWizardOpen, setMigrationWizardOpen] = useState(false);

  // ---------- 存储源状态 ----------
  const [sources, setSources] = useState<StorageSourceInfo[]>([]);
  const [sourcesLoading, setSourcesLoading] = useState(false);
  const [editingSource, setEditingSource] = useState<StorageSourceInfo | null>(null);
  const [sourceModalOpen, setSourceModalOpen] = useState(false);
  const [sourceForm] = Form.useForm();

  // ---------- 默认程序状态 ----------
  const [defaultApps, setDefaultApps] = useState<Record<string, DefaultAppConfig>>({});
  const [defaultAppsLoading, setDefaultAppsLoading] = useState(false);
  const [appModalOpen, setAppModalOpen] = useState(false);
  const [editingAppType, setEditingAppType] = useState<ReferenceType | null>(null);
  const [appForm] = Form.useForm();

  // ---------- 通用（启动默认页）状态（M7-3） ----------
  const [defaultHome, setDefaultHome] = useState<DefaultHome>("dashboard");
  const [defaultHomeLoading, setDefaultHomeLoading] = useState(false);
  const [defaultHomeSaving, setDefaultHomeSaving] = useState(false);

  // ---------- 加载数据 ----------

  const loadDirs = useCallback(async () => {
    setDirsLoading(true);
    try {
      const list = await inboxGetWatchDirs();
      setDirs(list.sort((a, b) => a.path.localeCompare(b.path)));
    } catch (err) {
      const apiErr = toApiError(err);
      message.error(`加载监控目录失败：${apiErr.message}`);
    } finally {
      setDirsLoading(false);
    }
  }, []);

  const loadRootDir = useCallback(async () => {
    setRootDirLoading(true);
    try {
      const status = await settingsGetRootDir();
      setRootDirStatus(status);
    } catch (err) {
      const apiErr = toApiError(err);
      message.error(`加载根目录失败：${apiErr.message}`);
    } finally {
      setRootDirLoading(false);
    }
  }, []);

  const loadSources = useCallback(async () => {
    setSourcesLoading(true);
    try {
      const list = await settingsListSources();
      setSources(list);
    } catch (err) {
      const apiErr = toApiError(err);
      message.error(`加载存储源失败：${apiErr.message}`);
    } finally {
      setSourcesLoading(false);
    }
  }, []);

  const loadDefaultApps = useCallback(async () => {
    setDefaultAppsLoading(true);
    try {
      const entries = await Promise.all(
        REFERENCE_TYPES.map(async ({ key }) => {
          const cfg = await settingsGetDefaultApp(key);
          return [key, cfg] as const;
        }),
      );
      setDefaultApps(Object.fromEntries(entries));
    } catch (err) {
      const apiErr = toApiError(err);
      message.error(`加载默认程序失败：${apiErr.message}`);
    } finally {
      setDefaultAppsLoading(false);
    }
  }, []);

  const loadDefaultHome = useCallback(async () => {
    setDefaultHomeLoading(true);
    try {
      const cfg = await settingsGetDefaultHome();
      setDefaultHome(cfg.home);
    } catch (err) {
      const apiErr = toApiError(err);
      message.error(`加载启动默认页失败：${apiErr.message}`);
    } finally {
      setDefaultHomeLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadDirs();
    void loadRootDir();
    void loadSources();
    void loadDefaultApps();
    void loadDefaultHome();
  }, [loadDirs, loadRootDir, loadSources, loadDefaultApps, loadDefaultHome]);

  // ---------- 监控目录操作 ----------

  async function handleAddSubmit() {
    try {
      const values = await addForm.validateFields();
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

  // ---------- 存储源操作 ----------

  function handleEditSource(source: StorageSourceInfo) {
    setEditingSource(source);
    sourceForm.setFieldsValue({ name: source.name });
    setSourceModalOpen(true);
  }

  async function handleSourceSubmit() {
    try {
      const values = await sourceForm.validateFields();
      if (!editingSource) return;
      setSubmitting(true);
      await settingsUpdateSource({ id: editingSource.id, name: values.name });
      message.success("存储源名称已更新");
      setSourceModalOpen(false);
      setEditingSource(null);
      void loadSources();
    } catch (err) {
      if (err && typeof err === "object" && "errorFields" in err) {
        return;
      }
      const apiErr = toApiError(err);
      message.error(`更新失败：${apiErr.message}`);
    } finally {
      setSubmitting(false);
    }
  }

  // ---------- 默认程序操作 ----------

  function handleEditApp(type: ReferenceType) {
    setEditingAppType(type);
    const cfg = defaultApps[type];
    appForm.setFieldsValue({
      strategy: cfg?.strategy ?? "system_default",
      appPath: cfg?.appPath ?? "",
    });
    setAppModalOpen(true);
  }

  // ---------- 通用（启动默认页）操作（M7-3） ----------

  async function handleDefaultHomeChange(value: DefaultHome) {
    setDefaultHomeSaving(true);
    try {
      const cfg = await settingsSetDefaultHome(value);
      setDefaultHome(cfg.home);
      message.success("启动默认页已更新，重启应用后生效");
    } catch (err) {
      const apiErr = toApiError(err);
      message.error(`设置失败：${apiErr.message}`);
    } finally {
      setDefaultHomeSaving(false);
    }
  }

  async function handleAppSubmit() {
    try {
      const values = await appForm.validateFields();
      if (!editingAppType) return;
      setSubmitting(true);
      await settingsSetDefaultApp({
        type: editingAppType,
        strategy: values.strategy,
        appPath: values.strategy === "app" ? values.appPath : undefined,
      });
      message.success("默认程序已更新");
      setAppModalOpen(false);
      setEditingAppType(null);
      void loadDefaultApps();
    } catch (err) {
      if (err && typeof err === "object" && "errorFields" in err) {
        return;
      }
      const apiErr = toApiError(err);
      message.error(`设置失败：${apiErr.message}`);
    } finally {
      setSubmitting(false);
    }
  }

  // ---------- 渲染 ----------

  return (
    <div style={{ maxWidth: 800, margin: "0 auto" }}>
      <Typography.Title level={3}>设置</Typography.Title>

      {/* 通用分区（M7-3 · 启动默认页） */}
      <Card title="通用" style={{ marginBottom: 16 }} loading={defaultHomeLoading}>
        <Space direction="vertical" size={8} style={{ width: "100%" }}>
          <Space align="center" style={{ width: "100%", justifyContent: "space-between" }}>
            <Typography.Text>启动时打开</Typography.Text>
            <Select<DefaultHome>
              value={defaultHome}
              onChange={(v) => void handleDefaultHomeChange(v)}
              loading={defaultHomeSaving}
              style={{ minWidth: 160 }}
              options={[
                { value: "dashboard", label: "工作台" },
                { value: "spaces", label: "空间" },
              ]}
            />
          </Space>
          <Typography.Text type="secondary" style={{ fontSize: 12 }}>
            应用启动后默认展示的页面；修改后下次启动生效。
          </Typography.Text>
        </Space>
      </Card>

      {/* 根目录分区 */}
      <Card
        title="根目录"
        style={{ marginBottom: 16 }}
        extra={
          <Button
            icon={<SettingOutlined />}
            onClick={() => setMigrationWizardOpen(true)}
            disabled={!rootDirStatus?.initialized}
          >
            修改根目录
          </Button>
        }
        loading={rootDirLoading}
      >
        {rootDirStatus?.initialized && rootDirStatus.rootDir ? (
          <Space direction="vertical" size={8} style={{ width: "100%" }}>
            <Typography.Text strong>{rootDirStatus.rootDir}</Typography.Text>
            <Space size={8} wrap>
              {TYPE_SUBDIRS.map((subdir) => (
                <Tag key={subdir} color="green">
                  {subdir}
                </Tag>
              ))}
            </Space>
          </Space>
        ) : (
          <Empty description="根目录未初始化" />
        )}
      </Card>

      {/* 监控目录分区 */}
      <Card
        title="监控目录"
        style={{ marginBottom: 16 }}
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
          loading={dirsLoading}
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

      {/* 存储源分区 */}
      <Card title="存储源" style={{ marginBottom: 16 }} loading={sourcesLoading}>
        <List<StorageSourceInfo>
          dataSource={sources}
          rowKey="id"
          locale={{ emptyText: <Empty description="暂无存储源" /> }}
          renderItem={(source) => (
            <List.Item
              actions={[
                <Button
                  key="edit"
                  type="text"
                  icon={<EditOutlined />}
                  onClick={() => handleEditSource(source)}
                >
                  编辑
                </Button>,
              ]}
            >
              <List.Item.Meta
                title={source.name}
                description={
                  <Space direction="vertical" size={4}>
                    <Typography.Text type="secondary" style={{ fontSize: 12 }}>
                      类型：{source.kind} · 状态：{source.status}
                    </Typography.Text>
                    <Space size={4} wrap>
                      <Tag color={source.capabilities.archive ? "green" : "default"}>
                        archive
                      </Tag>
                      <Tag color={source.capabilities.softDelete ? "green" : "default"}>
                        softDelete
                      </Tag>
                      <Tag color={source.capabilities.destroy ? "green" : "default"}>
                        destroy
                      </Tag>
                      <Tag color={source.capabilities.restoreFromBin ? "green" : "default"}>
                        restoreFromBin
                      </Tag>
                    </Space>
                  </Space>
                }
              />
            </List.Item>
          )}
        />
      </Card>

      {/* 默认程序分区 */}
      <Card title="默认程序" loading={defaultAppsLoading}>
        <List
          dataSource={REFERENCE_TYPES}
          rowKey="key"
          renderItem={({ key, label }) => {
            const cfg = defaultApps[key];
            const isCustom = cfg?.strategy === "app";
            return (
              <List.Item
                actions={[
                  <Button
                    key="set"
                    type="text"
                    icon={<SettingOutlined />}
                    onClick={() => handleEditApp(key)}
                  >
                    设置
                  </Button>,
                ]}
              >
                <List.Item.Meta
                  title={label}
                  description={
                    isCustom ? (
                      <Typography.Text type="secondary" style={{ fontSize: 12 }}>
                        {cfg.appPath}
                      </Typography.Text>
                    ) : (
                      <Typography.Text type="secondary" style={{ fontSize: 12 }}>
                        系统默认
                      </Typography.Text>
                    )
                  }
                />
              </List.Item>
            );
          }}
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
        <Form form={addForm} layout="vertical">
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
              value={addForm.getFieldValue("path") || ""}
              onChange={(path) => addForm.setFieldValue("path", path)}
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

      {/* 编辑存储源对话框 */}
      <Modal
        title="编辑存储源"
        open={sourceModalOpen}
        onOk={handleSourceSubmit}
        onCancel={() => {
          setSourceModalOpen(false);
          setEditingSource(null);
        }}
        confirmLoading={submitting}
        okText="保存"
        cancelText="取消"
        destroyOnHidden
      >
        <Form form={sourceForm} layout="vertical">
          <Form.Item
            name="name"
            label="名称"
            rules={[{ required: true, message: "请输入存储源名称" }]}
          >
            <Input placeholder="存储源显示名称" />
          </Form.Item>
        </Form>
      </Modal>

      {/* 设置默认程序对话框 */}
      <Modal
        title={`设置默认程序 — ${REFERENCE_TYPES.find((t) => t.key === editingAppType)?.label ?? editingAppType}`}
        open={appModalOpen}
        onOk={handleAppSubmit}
        onCancel={() => {
          setAppModalOpen(false);
          setEditingAppType(null);
        }}
        confirmLoading={submitting}
        okText="保存"
        cancelText="取消"
        destroyOnHidden
      >
        <Form form={appForm} layout="vertical">
          <Form.Item name="strategy" label="打开方式">
            <Radio.Group>
              <Radio value="system_default">系统默认</Radio>
              <Radio value="app">指定程序</Radio>
            </Radio.Group>
          </Form.Item>
          <Form.Item
            noStyle
            shouldUpdate={(prev, curr) => prev.strategy !== curr.strategy}
          >
            {({ getFieldValue }) =>
              getFieldValue("strategy") === "app" ? (
                <Form.Item
                  name="appPath"
                  label="程序路径"
                  rules={[
                    { required: true, message: "请输入程序路径" },
                    {
                      pattern: /^\/.*/,
                      message: "请输入绝对路径（以 / 开头）",
                    },
                  ]}
                >
                  <Input placeholder="例如 /Applications/Typora.app" />
                </Form.Item>
              ) : null
            }
          </Form.Item>
        </Form>
      </Modal>

      {/* 根目录迁移向导 — M6-6.6 */}
      <MigrationWizard
        open={migrationWizardOpen}
        onClose={() => setMigrationWizardOpen(false)}
        onSuccess={() => {
          setMigrationWizardOpen(false);
          message.success("根目录已更新");
          void loadRootDir();
        }}
      />
    </div>
  );
}
