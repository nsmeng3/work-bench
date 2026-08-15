import { useState } from "react";
import { Button, Input, Space, message } from "antd";
import { FolderOpenOutlined } from "@ant-design/icons";

interface RootDirPickerProps {
  /** 当前路径值 */
  value: string;
  /** 路径变化回调（手动输入或浏览选择均触发） */
  onChange: (path: string) => void;
  /** 输入框占位符 */
  placeholder?: string;
  /** 是否禁用（如提交中） */
  disabled?: boolean;
}

/**
 * 根目录选择器 — 输入框 + 「浏览…」按钮组合。
 *
 * 通过 `@tauri-apps/plugin-dialog` 调系统目录选择器；
 * 在非 Tauri 环境 / 插件未注册 / 用户取消时降级为手动输入。
 *
 * 命名与形态与 M4 设置中心「根目录迁移」入口复用约定保持一致。
 */
export function RootDirPicker({
  value,
  onChange,
  placeholder = "请选择或输入绝对路径，例如 /Users/you/Workbench",
  disabled = false,
}: RootDirPickerProps) {
  const [messageApi, contextHolder] = message.useMessage();
  const [picking, setPicking] = useState(false);

  async function handleBrowse() {
    setPicking(true);
    try {
      const dialog = await import("@tauri-apps/plugin-dialog");
      const selected = await dialog.open({
        directory: true,
        multiple: false,
      });
      if (typeof selected === "string" && selected) {
        onChange(selected);
      }
      // selected === null 表示用户取消，保持原值
    } catch (err) {
      // 插件未注册 / 非 Tauri 环境：降级提示手动输入
      console.warn("tauri dialog 不可用，降级为手动输入：", err);
      messageApi.info("系统目录选择器不可用，请手动输入绝对路径");
    } finally {
      setPicking(false);
    }
  }

  return (
    <Space.Compact style={{ width: "100%" }}>
      {contextHolder}
      <Input
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        disabled={disabled}
        allowClear
      />
      <Button
        icon={<FolderOpenOutlined />}
        onClick={handleBrowse}
        loading={picking}
        disabled={disabled}
      >
        浏览…
      </Button>
    </Space.Compact>
  );
}
