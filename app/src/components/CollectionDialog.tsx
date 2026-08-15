import { useEffect } from "react";
import { Modal, Form, Input, Select, message } from "antd";
import type { Collection, CollectionCreateInput, CollectionUpdateInput } from "../api";
import { collectionCreate, collectionUpdate, toApiError } from "../api";

const NAME_MAX_LENGTH = 64;

interface CollectionDialogProps {
  open: boolean;
  /** 所属空间 id（创建时必填） */
  spaceId: string;
  /** 传入则为编辑模式，否则为创建模式 */
  collection?: Collection;
  onClose: () => void;
  onSaved: () => void;
}

interface CollectionFormValues {
  name: string;
  summary?: string;
  tags?: string[];
}

export function CollectionDialog({ open, spaceId, collection, onClose, onSaved }: CollectionDialogProps) {
  const isEdit = !!collection;
  const [form] = Form.useForm<CollectionFormValues>();
  const [messageApi, contextHolder] = message.useMessage();

  useEffect(() => {
    if (open) {
      form.setFieldsValue({
        name: collection?.name ?? "",
        summary: collection?.summary ?? "",
        tags: collection?.tags ?? [],
      });
    } else {
      form.resetFields();
    }
  }, [open, collection, form]);

  async function handleOk() {
    const values = await form.validateFields();
    const nameTrimmed = values.name.trim();
    const payload = {
      name: nameTrimmed,
      summary: values.summary?.trim() || undefined,
      tags: values.tags && values.tags.length > 0 ? values.tags : undefined,
    };

    try {
      if (isEdit) {
        const input: CollectionUpdateInput = { id: collection.id, ...payload };
        await collectionUpdate(input);
        messageApi.success("资源集已更新");
      } else {
        const input: CollectionCreateInput = { spaceId, ...payload };
        await collectionCreate(input);
        messageApi.success("资源集已创建");
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
        title={isEdit ? "编辑资源集" : "创建资源集"}
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
            <Input placeholder="资源集名称" autoFocus maxLength={NAME_MAX_LENGTH + 10} />
          </Form.Item>

          <Form.Item name="summary" label="简介">
            <Input.TextArea placeholder="可选简介" rows={3} />
          </Form.Item>

          <Form.Item name="tags" label="标签">
            <Select mode="tags" placeholder="输入后回车添加标签" tokenSeparators={[","]} />
          </Form.Item>
        </Form>
      </Modal>
    </>
  );
}
