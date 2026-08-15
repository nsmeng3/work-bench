import { useState } from "react";
import { Alert, Button, Card, Collapse, Form, Typography } from "antd";
import { CheckCircleOutlined } from "@ant-design/icons";
import { RootDirPicker } from "../components/RootDirPicker";
import { settingsInitRootDir, toApiError } from "../api";
import type { ApiError, InitRootDirResult } from "../api";

const { Title, Paragraph, Text } = Typography;

/** 与后端 settings.rs 中 TYPE_SUBDIRS 保持一致 */
const TYPE_SUBDIRS = ["Code", "Documents", "Data", "Artifacts", "Tools", "Media"] as const;

interface InitWizardPageProps {
  /** 初始化成功回调 — 由外层路由切回主界面 */
  onInitialized: (rootDir: string) => void;
}

interface InitFormValues {
  rootDir: string;
}

/**
 * 首次启动初始化向导 — 详细设计说明书 §4。
 *
 * 单页流程：
 *  1. 说明文案 + 6 个子目录列表
 *  2. RootDirPicker 选择/输入根目录
 *  3. 「初始化」按钮调 settings_init_root_dir，loading 态
 *  4. 成功展示 created 列表（可折叠），由用户确认后跳回主界面
 *  5. 失败按统一错误模型展示 message；retryable=true 时给重试按钮
 */
export function InitWizardPage({ onInitialized }: InitWizardPageProps) {
  const [form] = Form.useForm<InitFormValues>();
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<ApiError | null>(null);
  const [result, setResult] = useState<InitRootDirResult | null>(null);

  async function handleSubmit(values: InitFormValues) {
    const rootDir = values.rootDir.trim();
    setSubmitting(true);
    setError(null);
    try {
      const r = await settingsInitRootDir({ rootDir });
      setResult(r);
    } catch (err) {
      setError(toApiError(err));
    } finally {
      setSubmitting(false);
    }
  }

  function handleRetry() {
    // 重试：直接复用当前表单值再走一次提交
    form.submit();
  }

  /* ---------- 成功态 ---------- */
  if (result) {
    return (
      <div
        style={{
          minHeight: "100vh",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          padding: 24,
        }}
      >
        <Card style={{ maxWidth: 640, width: "100%" }}>
          <Title level={3}>
            <CheckCircleOutlined style={{ color: "#52c41a", marginRight: 8 }} />
            初始化完成
          </Title>
          <Paragraph>
            资源根目录已就绪：<Text code>{result.rootDir}</Text>
          </Paragraph>
          <Collapse
            style={{ marginBottom: 24 }}
            items={[
              {
                key: "created",
                label: `查看已创建的 ${result.created.length} 个子目录`,
                children: (
                  <ul style={{ paddingLeft: 20, marginBottom: 0 }}>
                    {result.created.map((p) => (
                      <li key={p}>
                        <Text code>{p}</Text>
                      </li>
                    ))}
                  </ul>
                ),
              },
            ]}
          />
          <Button type="primary" block onClick={() => onInitialized(result.rootDir)}>
            进入主界面
          </Button>
        </Card>
      </div>
    );
  }

  /* ---------- 表单态 ---------- */
  return (
    <div
      style={{
        minHeight: "100vh",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        padding: 24,
      }}
    >
      <Card style={{ maxWidth: 640, width: "100%" }}>
        <Title level={3}>初始化资源根目录</Title>
        <Paragraph>
          首次使用需要选择一个资源根目录，用于集中存放工作台管理的所有资源。
          将在其下创建以下 6 个子目录：
        </Paragraph>
        <Paragraph>
          <ul style={{ paddingLeft: 20 }}>
            {TYPE_SUBDIRS.map((d) => (
              <li key={d}>
                <Text code>{d}</Text>
              </li>
            ))}
          </ul>
        </Paragraph>

        {error && (
          <Alert
            style={{ marginBottom: 16 }}
            type="error"
            showIcon
            message={error.message}
            description={
              error.code ? (
                <Text type="secondary" style={{ fontSize: 12 }}>
                  错误码：{error.code}
                </Text>
              ) : undefined
            }
            action={
              error.retryable ? (
                <Button size="small" onClick={handleRetry} loading={submitting}>
                  重试
                </Button>
              ) : undefined
            }
            closable
            onClose={() => setError(null)}
          />
        )}

        <Form<InitFormValues>
          form={form}
          layout="vertical"
          onFinish={handleSubmit}
          initialValues={{ rootDir: "" }}
        >
          <Form.Item
            name="rootDir"
            label="资源根目录"
            rules={[
              { required: true, message: "请选择或输入资源根目录" },
              {
                validator: (_, v: string) => {
                  const t = (v ?? "").trim();
                  if (!t) return Promise.resolve();
                  if (!t.startsWith("/")) {
                    return Promise.reject(new Error("请输入绝对路径（以 / 开头）"));
                  }
                  return Promise.resolve();
                },
              },
            ]}
          >
            <RootDirPickerBridge disabled={submitting} />
          </Form.Item>

          <Form.Item style={{ marginBottom: 0 }}>
            <Button type="primary" htmlType="submit" block loading={submitting}>
              初始化
            </Button>
          </Form.Item>
        </Form>
      </Card>
    </div>
  );
}

/**
 * Form.Item 与受控 RootDirPicker 之间的桥接：
 * antd Form 会向子组件注入 value / onChange，RootDirPicker 已按此契约设计。
 */
function RootDirPickerBridge({
  value,
  onChange,
  disabled,
}: {
  value?: string;
  onChange?: (v: string) => void;
  disabled?: boolean;
}) {
  return (
    <RootDirPicker
      value={value ?? ""}
      onChange={(v) => onChange?.(v)}
      disabled={disabled}
    />
  );
}
