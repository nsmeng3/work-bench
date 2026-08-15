import { Form, Input, Radio, Select, Space as AntSpace } from "antd";
import type {
  ReferenceConfidentiality,
  ReferenceLifecycle,
  ReferenceType,
} from "../api";

/**
 * 引用创建/编辑共享的基础字段子组件（M2-2.10 抽取）。
 *
 * 覆盖字段：type / name / description / tags / lifecycle / confidentiality。
 * 不包含 path（locator）字段 —— 各创建方式（external / managed）对路径的
 * 文案与辅助按钮不同，由各自对话框自行渲染。
 *
 * 使用方需自行创建 Form 实例并传入；本组件只渲染 Form.Item 列表。
 */

export const NAME_MAX_LENGTH = 128;

export const TYPE_OPTIONS: { value: ReferenceType; label: string }[] = [
  { value: "code", label: "代码" },
  { value: "document", label: "文档" },
  { value: "data", label: "数据" },
  { value: "artifact", label: "构建产物" },
  { value: "tool", label: "工具" },
  { value: "media", label: "媒体" },
];

export const LIFECYCLE_OPTIONS: { value: ReferenceLifecycle; label: string }[] = [
  { value: "active", label: "活跃" },
  { value: "staged", label: "已暂存" },
  { value: "delivered", label: "已交付" },
  { value: "archived", label: "已归档" },
];

export const CONFIDENTIALITY_OPTIONS: { value: ReferenceConfidentiality; label: string }[] = [
  { value: "public", label: "公开" },
  { value: "internal", label: "内部" },
  { value: "customer_restricted", label: "客户受限" },
  { value: "sensitive", label: "敏感" },
];

export function ReferenceBaseFields() {
  return (
    <>
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
    </>
  );
}
