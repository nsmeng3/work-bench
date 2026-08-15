import { useEffect } from "react";
import { Modal, Form, Input } from "antd";
import type { Space, SpaceCreateInput, SpaceUpdateInput } from "../api";
import { spaceCreate, spaceUpdate, toApiError } from "../api";
import { message } from "antd";

const NAME_MAX_LENGTH = 64;

interface SpaceDialogProps {
  open: boolean;
  /** 传入则为编辑模式，否则为创建模式 */
  space?: Space;
  onClose: () => void;
  onSaved: () => void;
}

interface SpaceFormValues {
  name: string;
  description?: string;
  color?: string;
  icon?: string;
}

export function SpaceDialog({ open, space, onClose, onSaved }: SpaceDialogProps) {
  const isEdit = !!space;
  const [form] = Form.useForm<SpaceFormValues>();
  const [messageApi, contextHolder] = message.useMessage();

  useEffect(() => {
    if (open) {
      form.setFieldsValue({
        name: space?.name ?? "",
        description: space?.description ?? "",
        color: space?.color ?? "",
        icon: space?.icon ?? "",
      });
    } else {
      form.resetFields();
    }
  }, [open, space, form]);

  async function handleOk() {
    const values = await form.validateFields();
    const nameTrimmed = values.name.trim();
    const payload = {
      name: nameTrimmed,
      description: values.description?.trim() || undefined,
      color: values.color || undefined,
      icon: values.icon || undefined,
    };

    try {
      if (isEdit) {
        const input: SpaceUpdateInput = { id: space.id, ...payload };
        await spaceUpdate(input);
        messageApi.success("空间已更新");
      } else {
        const input: SpaceCreateInput = payload;
        await spaceCreate(input);
        messageApi.success("空间已创建");
      }
      onSaved();
      onClose();
    } catch (err) {
      const apiErr = toApiError(err);
      messageApi.error(apiErr.message);
      // 不关闭弹窗，允许用户修正后重试
      throw err;
    }
  }

  return (
    <>
      {contextHolder}
      <Modal
        title={isEdit ? "编辑空间" : "创建空间"}
        open={open}
        onCancel={onClose}
        onOk={handleOk}
        okText={isEdit ? "保存" : "创建"}
        cancelText="取消"
        destroyOnHidden
        maskClosable={false}
      >
        <Form form={form} layout="vertical" preserve={false}>
          <Form.Item
            name="name"
            label="名称"
            rules={[
              {
                required: true,
                whitespace: true,
                message: "名称不能为空",
              },
              {
                max: NAME_MAX_LENGTH,
                message: `名称超长（最大 ${NAME_MAX_LENGTH} 字符）`,
              },
            ]}
          >
            <Input placeholder="空间名称" autoFocus maxLength={NAME_MAX_LENGTH + 10} />
          </Form.Item>

          <Form.Item name="description" label="描述">
            <Input.TextArea placeholder="可选描述" rows={3} />
          </Form.Item>

          <Form.Item name="color" label="颜色">
            <Input placeholder="如 #4A90D9" />
          </Form.Item>

          <Form.Item name="icon" label="图标">
            <Input placeholder="图标 key" />
          </Form.Item>
        </Form>
      </Modal>
    </>
  );
}
