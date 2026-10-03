import { useEffect, useState } from "react";
import { Modal, Form, Input, Select, Radio, Button, Space as AntSpace, Typography, message } from "antd";
import { FolderOpenOutlined, FileOutlined } from "@ant-design/icons";
import type {
  RefCreateExternalInput,
  ReferenceConfidentiality,
  ReferenceLifecycle,
  ReferenceType,
} from "../api";
import { refCreateExternal, toApiError } from "../api";

const { Text } = Typography;

const NAME_MAX_LENGTH = 128;

const TYPE_OPTIONS: { value: ReferenceType; label: string }[] = [
  { value: "code", label: "代码" },
  { value: "document", label: "文档" },
  { value: "data", label: "数据" },
  { value: "artifact", label: "构建产物" },
  { value: "tool", label: "工具" },
  { value: "media", label: "媒体" },
];

const LIFECYCLE_OPTIONS: { value: ReferenceLifecycle; label: string }[] = [
  { value: "active", label: "活跃" },
  { value: "staged", label: "已暂存" },
  { value: "delivered", label: "已交付" },
  { value: "archived", label: "已归档" },
];

const CONFIDENTIALITY_OPTIONS: { value: ReferenceConfidentiality; label: string }[] = [
  { value: "public", label: "公开" },
  { value: "internal", label: "内部" },
  { value: "customer_restricted", label: "客户受限" },
  { value: "sensitive", label: "敏感" },
];

interface ReferenceCreateDialogProps {
  open: boolean;
  /** 所属资源集 id */
  collectionId: string;
  /** 打开时「类型」字段的预选值（跟随资源模块左侧选中类型）；缺省 "document" */
  defaultType?: ReferenceType;
  onClose: () => void;
  /** 创建成功后回调（父组件应重新拉取 collection_get） */
  onCreated: () => void;
}

interface ReferenceFormValues {
  path: string;
  type: ReferenceType;
  name: string;
  description?: string;
  tags?: string[];
  lifecycle: ReferenceLifecycle;
  confidentiality: ReferenceConfidentiality;
}

/**
 * 通过 Tauri dialog 选择文件/目录。
 *
 * 注意：`@tauri-apps/plugin-dialog` 已在 dependencies 中，但 Rust 侧（src-tauri）
 * 当前未注册该插件，因此真实环境下 open() 会抛错。此处捕获异常并返回 null，
 * 由调用方提示用户改用手动输入路径。MOCK 模式下同样 fallback 到手填。
 */
async function pickPath(kind: "file" | "directory"): Promise<string | null> {
  try {
    const dialog = await import("@tauri-apps/plugin-dialog");
    const selected = await dialog.open({
      directory: kind === "directory",
      multiple: false,
    });
    if (typeof selected === "string") return selected;
    return null;
  } catch (err) {
    // 插件未注册 / 非 Tauri 环境 / 用户取消外的其他错误统一降级
    console.warn("tauri dialog 不可用，降级为手动输入：", err);
    return null;
  }
}

export function ReferenceCreateDialog({
  open,
  collectionId,
  defaultType,
  onClose,
  onCreated,
}: ReferenceCreateDialogProps) {
  const [form] = Form.useForm<ReferenceFormValues>();
  const [messageApi, contextHolder] = message.useMessage();
  const [submitting, setSubmitting] = useState(false);

  useEffect(() => {
    if (open) {
      form.setFieldsValue({
        path: "",
        type: defaultType ?? "document",
        name: "",
        description: "",
        tags: [],
        lifecycle: "active",
        confidentiality: "internal",
      });
    } else {
      form.resetFields();
    }
  }, [open, defaultType, form]);

  async function handlePick(kind: "file" | "directory") {
    const path = await pickPath(kind);
    if (path) {
      form.setFieldsValue({ path });
      // 若名称未填，用路径末段做默认名
      const currentName = form.getFieldValue("name") as string | undefined;
      if (!currentName || currentName.trim() === "") {
        const segments = path.split("/").filter(Boolean);
        const base = segments[segments.length - 1] ?? path;
        form.setFieldsValue({ name: base });
      }
    } else {
      messageApi.info("系统文件选择器不可用，请手动输入绝对路径");
    }
  }

  async function handleOk() {
    const values = await form.validateFields();
    const nameTrimmed = values.name.trim();
    const pathTrimmed = values.path.trim();

    const input: RefCreateExternalInput = {
      collectionId,
      name: nameTrimmed,
      type: values.type,
      locator: { kind: "path", path: pathTrimmed },
      description: values.description?.trim() || undefined,
      tags: values.tags && values.tags.length > 0 ? values.tags : undefined,
      lifecycle: values.lifecycle,
      confidentiality: values.confidentiality,
      // indexed 缺省，由后端默认 true（§2.5）
    };

    setSubmitting(true);
    try {
      await refCreateExternal(input);
      messageApi.success("引用已创建");
      onCreated();
      onClose();
    } catch (err) {
      const apiErr = toApiError(err);
      if (apiErr.code === "FS_PATH_NOT_FOUND") {
        messageApi.error(`路径不存在：${pathTrimmed}。请检查路径后重试。`);
      } else {
        messageApi.error(apiErr.retryable ? `${apiErr.message}（可重试）` : apiErr.message);
      }
      // 不关闭弹窗，允许用户修正后重试
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <>
      {contextHolder}
      <Modal
        title="添加引用（仅关联，不改动原文件）"
        open={open}
        onCancel={onClose}
        onOk={handleOk}
        okText="创建"
        cancelText="取消"
        confirmLoading={submitting}
        destroyOnHidden
        maskClosable={false}
        width={560}
      >
        <Form form={form} layout="vertical" preserve={false}>
          <Form.Item
            name="path"
            label="文件 / 目录路径"
            rules={[
              { required: true, whitespace: true, message: "路径不能为空" },
              {
                validator: (_, v: string) =>
                  v && v.trim().startsWith("/")
                    ? Promise.resolve()
                    : Promise.reject(new Error("请输入绝对路径（以 / 开头）")),
              },
            ]}
            extra={
              <Text type="secondary" style={{ fontSize: 12 }}>
                仅登记路径，不会复制 / 移动 / 修改原文件
              </Text>
            }
          >
            <Input
              placeholder="/abs/path/to/resource"
              allowClear
              addonAfter={
                <AntSpace size={4}>
                  <Button
                    size="small"
                    type="text"
                    icon={<FileOutlined />}
                    onClick={() => handlePick("file")}
                  >
                    选文件
                  </Button>
                  <Button
                    size="small"
                    type="text"
                    icon={<FolderOpenOutlined />}
                    onClick={() => handlePick("directory")}
                  >
                    选目录
                  </Button>
                </AntSpace>
              }
            />
          </Form.Item>

          <Form.Item name="type" label="类型" rules={[{ required: true, message: "请选择类型" }]}>
            <Radio.Group
              options={TYPE_OPTIONS}
              optionType="button"
              buttonStyle="solid"
              style={{ flexWrap: "wrap", display: "flex", gap: 4 }}
            />
          </Form.Item>

          <Form.Item
            name="name"
            label="名称"
            rules={[
              { required: true, whitespace: true, message: "名称不能为空" },
              { max: NAME_MAX_LENGTH, message: `名称超长（最大 ${NAME_MAX_LENGTH} 字符）` },
            ]}
          >
            <Input placeholder="引用名称" maxLength={NAME_MAX_LENGTH + 10} />
          </Form.Item>

          <Form.Item name="description" label="描述">
            <Input.TextArea placeholder="可选描述" rows={2} />
          </Form.Item>

          <Form.Item name="tags" label="标签">
            <Select mode="tags" placeholder="输入后回车添加标签" tokenSeparators={[","]} />
          </Form.Item>

          <AntSpace size={16} style={{ display: "flex" }} wrap>
            <Form.Item
              name="lifecycle"
              label="生命周期"
              style={{ marginBottom: 0, minWidth: 200 }}
              rules={[{ required: true }]}
            >
              <Select options={LIFECYCLE_OPTIONS} />
            </Form.Item>
            <Form.Item
              name="confidentiality"
              label="保密级别"
              style={{ marginBottom: 0, minWidth: 200 }}
              rules={[{ required: true }]}
            >
              <Select options={CONFIDENTIALITY_OPTIONS} />
            </Form.Item>
          </AntSpace>
        </Form>
      </Modal>
    </>
  );
}
