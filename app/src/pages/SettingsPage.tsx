import { useCallback, useEffect, useRef, useState } from "react";
import {
  Anchor,
  Button,
  Card,
  Divider,
  Empty,
  Form,
  Input,
  List,
  Modal,
  Radio,
  Select,
  Space,
  Switch,
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
  settingsGetLaunchAtLogin,
  settingsSetLaunchAtLogin,
  toApiError,
} from "../api";
import { RootDirPicker } from "../components/RootDirPicker";
import { MigrationWizard } from "../components/MigrationWizard";

/**
 * 设置页 — 左侧锚点定位菜单 + 右侧独立滚动（双栏布局）。
 *
 * 分区（按类型聚合，锚点与之一一对应）：
 * 1. 通用：启动默认页、开机自启
 * 2. 存储：根目录 + 存储源（数据存放相关合并）
 * 3. 监控：监控目录
 * 4. 打开方式：按 6 种资源类型配置默认打开方式
 *
 * 滚动模式与收件箱一致：右栏 flex item 由交叉轴 stretch 定界 + overflowY:auto；
 * 左栏固定不滚。Anchor.getContainer 指向右栏，点击定位、滚动高亮跟随。
 */

/** 设置分区锚点定义（id 前缀 settings-） */
const SECTION_ANCHORS = [
  { key: "general", href: "#settings-general", title: "通用" },
  { key: "storage", href: "#settings-storage", title: "存储" },
  { key: "watch", href: "#settings-watch", title: "监控" },
  { key: "open-with", href: "#settings-open-with", title: "打开方式" },
];

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
  /** 右侧滚动容器 ref：Anchor 的定位目标容器 */
  const contentRef = useRef<HTMLDivElement>(null);

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

  // ---------- 通用（开机自启）状态 ----------
  const [launchAtLogin, setLaunchAtLogin] = useState(true);
  const [launchAtLoginSaving, setLaunchAtLoginSaving] = useState(false);

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
      const loginCfg = await settingsGetLaunchAtLogin();
      setLaunchAtLogin(loginCfg.enabled);
    } catch (err) {
      const apiErr = toApiError(err);
      message.error(`加载通用设置失败：${apiErr.message}`);
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
      const saved = await inboxSetWatchDir({
        id: crypto.randomUUID(),
        path: values.path,
        name: values.name || values.path.split("/").pop() || "未命名",
        description: values.description,
      });
      if (saved.paused === 1) {
        // 配置已保存但 watcher 挂载失败（典型：macOS 完全磁盘访问权限未授予）
        message.warning(
          "目录已保存，但启动监听失败：请在「系统设置 → 隐私与安全性 → 完全磁盘访问权限」中授权本应用后，重新保存该目录重试",
          8,
        );
      } else {
        message.success("监控目录添加成功，已即时开始监听");
      }
      setAddModalOpen(false);
      addForm.resetFields();
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

  async function handleLaunchAtLoginChange(checked: boolean) {
    setLaunchAtLoginSaving(true);
    try {
      const cfg = await settingsSetLaunchAtLogin(checked);
      setLaunchAtLogin(cfg.enabled);
      message.success(cfg.enabled ? "已开启开机自动启动" : "已关闭开机自动启动");
    } catch (err) {
      const apiErr = toApiError(err);
      message.error(`设置失败：${apiErr.message}`);
    } finally {
      setLaunchAtLoginSaving(false);
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
    <div style={{ height: "100%", display: "flex", gap: 16 }}>
      {/* 左侧定位菜单（固定不随内容滚动） */}
      <div style={{ width: 140, flexShrink: 0 }}>
        <Typography.Title level={3}>设置</Typography.Title>
        <Anchor
          affix={false}
          getContainer={() => contentRef.current as HTMLElement}
          items={SECTION_ANCHORS}
        />
      </div>

      {/* 右侧内容区（独立滚动；minWidth:0 防横向撑爆） */}
      <div
        ref={contentRef}
        style={{ flex: 1, minWidth: 0, height: "100%", overflowY: "auto", paddingRight: 8 }}
      >
        <div style={{ maxWidth: 800 }}>
          {/* 通用分区 */}
          <section id="settings-general" style={{ scrollMarginTop: 4 }}>
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
          <Space align="center" style={{ width: "100%", justifyContent: "space-between" }}>
            <Typography.Text>开机自动启动</Typography.Text>
            <Switch
              checked={launchAtLogin}
              loading={launchAtLoginSaving}
              onChange={(v) => void handleLaunchAtLoginChange(v)}
            />
          </Space>
          <Typography.Text type="secondary" style={{ fontSize: 12 }}>
            开启后登录系统时自动启动并驻留菜单栏（不弹出主窗口）。
          </Typography.Text>
              </Space>
            </Card>
          </section>

          {/* 存储分区：根目录 + 存储源（数据存放相关合并） */}
          <section id="settings-storage" style={{ scrollMarginTop: 4 }}>
            <Card title="存储" style={{ marginBottom: 16 }} loading={rootDirLoading || sourcesLoading}>
              <Space
                align="center"
                style={{ width: "100%", justifyContent: "space-between", marginBottom: 8 }}
              >
                <Typography.Text strong>根目录</Typography.Text>
                <Button
                  size="small"
                  icon={<SettingOutlined />}
                  onClick={() => setMigrationWizardOpen(true)}
                  disabled={!rootDirStatus?.initialized}
                >
                  修改根目录
                </Button>
              </Space>
              {rootDirStatus?.initialized && rootDirStatus.rootDir ? (
                <Space direction="vertical" size={8} style={{ width: "100%" }}>
                  <Typography.Text>{rootDirStatus.rootDir}</Typography.Text>
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

              <Divider style={{ margin: "16px 0" }} />

              <Typography.Text strong style={{ display: "block", marginBottom: 8 }}>
                存储源
              </Typography.Text>
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
          </section>

          {/* 监控分区 */}
          <section id="settings-watch" style={{ scrollMarginTop: 4 }}>
            <Card
              title="监控"
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
                      title={
                        <Space size={8}>
                          {dir.name || dir.path}
                          {dir.paused === 1 && (
                            <Tag color="red" title="监听挂载失败（可能缺少完全磁盘访问权限），重新保存该目录可重试">
                              已暂停
                            </Tag>
                          )}
                        </Space>
                      }
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
          </section>

          {/* 打开方式分区 */}
          <section id="settings-open-with" style={{ scrollMarginTop: 4 }}>
            <Card title="打开方式" style={{ marginBottom: 16 }} loading={defaultAppsLoading}>
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
          </section>
        </div>
      </div>

      {/* 添加目录对话框 */}
      <Modal
        title="添加监控目录"
        open={addModalOpen}
        onOk={handleAddSubmit}
        onCancel={() => {
          setAddModalOpen(false);
          addForm.resetFields();
        }}
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
