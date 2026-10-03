import { useEffect, useRef, useState } from "react";
import { Input, Modal, Select, Space as AntSpace, Tag, message } from "antd";
import type { InputRef } from "antd";
import { spaceList, todoCreate, toApiError } from "../api";
import type { Space, TodoPriority } from "../api";

interface QuickCaptureDialogProps {
  open: boolean;
  onClose: () => void;
}

/**
 * m8-8.1 · 快速记录弹窗（全局快捷键 Cmd/Ctrl+Shift+T 唤起）。
 *
 * 极简：标题（必填，回车即存）+ 空间（可选，默认全局）+ 优先级。
 * 保存后立即关闭，通过 message 反馈；Esc 直接关闭。
 */
export function QuickCaptureDialog({ open, onClose }: QuickCaptureDialogProps) {
  const [title, setTitle] = useState("");
  const [spaceId, setSpaceId] = useState<string | undefined>(undefined);
  const [priority, setPriority] = useState<TodoPriority>(0);
  const [spaces, setSpaces] = useState<Space[]>([]);
  const [saving, setSaving] = useState(false);
  const inputRef = useRef<InputRef>(null);
  const [messageApi, messageContextHolder] = message.useMessage();

  // 打开时：清空表单 + 加载空间 + 聚焦输入框
  useEffect(() => {
    if (!open) return;
    setTitle("");
    setSpaceId(undefined);
    setPriority(0);
    spaceList({ status: "active" })
      .then(setSpaces)
      .catch(() => void 0);
    // Modal 动画结束后聚焦
    const t = setTimeout(() => inputRef.current?.focus(), 120);
    return () => clearTimeout(t);
  }, [open]);

  async function handleSave() {
    const trimmed = title.trim();
    if (!trimmed) return;
    setSaving(true);
    try {
      await todoCreate({
        title: trimmed,
        spaceId,
        priority: priority > 0 ? priority : undefined,
      });
      messageApi.success({ content: "已记录", duration: 1.5 });
      onClose();
    } catch (err) {
      const apiErr = toApiError(err);
      messageApi.error({ content: apiErr.message, duration: 3 });
    } finally {
      setSaving(false);
    }
  }

  return (
    <Modal
      title={
        <AntSpace>
          快速记录
          <Tag color="default" style={{ fontWeight: 400 }}>
            ⌘⇧T
          </Tag>
        </AntSpace>
      }
      open={open}
      onCancel={onClose}
      onOk={() => void handleSave()}
      okText="记录"
      cancelText="取消"
      confirmLoading={saving}
      okButtonProps={{ disabled: !title.trim() }}
      width={440}
      destroyOnHidden
    >
      {messageContextHolder}
      <AntSpace direction="vertical" style={{ width: "100%" }} size={12}>
        <Input
          ref={inputRef}
          placeholder="要记什么？回车直接保存"
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          onPressEnter={() => void handleSave()}
          maxLength={200}
          disabled={saving}
        />
        <AntSpace style={{ width: "100%" }}>
          <Select
            style={{ minWidth: 180 }}
            allowClear
            placeholder="全局（不挂空间）"
            value={spaceId}
            onChange={(v) => setSpaceId(v)}
            options={spaces.map((s) => ({ value: s.id, label: s.name }))}
          />
          <Select
            style={{ minWidth: 100 }}
            value={priority}
            onChange={(v) => setPriority(v as TodoPriority)}
            options={[
              { value: 0, label: "普通" },
              { value: 1, label: "重要" },
              { value: 2, label: "紧急" },
            ]}
          />
        </AntSpace>
      </AntSpace>
    </Modal>
  );
}
