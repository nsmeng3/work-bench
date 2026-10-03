import { useEffect, useRef, useState } from "react";
import { Input, Modal, Select, Space as AntSpace, Tag, message } from "antd";
import type { InputRef } from "antd";
import { collectionList, spaceList, todoCreate, toApiError } from "../api";
import type { Collection, Space, TodoPriority } from "../api";

interface QuickCaptureDialogProps {
  open: boolean;
  onClose: () => void;
}

/**
 * m8-8.1 · 快速记录弹窗（全局快捷键 Cmd/Ctrl+Shift+T 唤起）。
 *
 * 极简：标题（必填，回车即存）+ 空间/资源集（可选，默认全局）+ 优先级。
 * 选中资源集时空间自动跟随（资源集本身就在空间下）。
 * 保存后立即关闭，通过 message 反馈；Esc 直接关闭。
 */
export function QuickCaptureDialog({ open, onClose }: QuickCaptureDialogProps) {
  const [title, setTitle] = useState("");
  const [spaceId, setSpaceId] = useState<string | undefined>(undefined);
  const [collectionId, setCollectionId] = useState<string | undefined>(undefined);
  const [priority, setPriority] = useState<TodoPriority>(0);
  const [spaces, setSpaces] = useState<Space[]>([]);
  const [collections, setCollections] = useState<Collection[]>([]);
  const [saving, setSaving] = useState(false);
  const inputRef = useRef<InputRef>(null);
  const [messageApi, messageContextHolder] = message.useMessage();

  // 打开时：清空表单 + 加载空间 + 聚焦输入框
  useEffect(() => {
    if (!open) return;
    setTitle("");
    setSpaceId(undefined);
    setCollectionId(undefined);
    setPriority(0);
    spaceList({ status: "active" })
      .then(async (list) => {
        setSpaces(list);
        // 全部资源集（按空间分组展示用）
        const cols = await Promise.all(
          list.map((s) => collectionList({ spaceId: s.id, status: "active" })),
        );
        setCollections(cols.flat());
      })
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
        collectionId,
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
        <AntSpace style={{ width: "100%" }} wrap>
          <Select
            style={{ minWidth: 150 }}
            allowClear
            placeholder="全局（不挂空间）"
            value={spaceId}
            onChange={(v) => {
              setSpaceId(v);
              // 空间变更后资源集不再匹配则清掉
              if (collectionId) {
                const col = collections.find((c) => c.id === collectionId);
                if (col && col.spaceId !== v) setCollectionId(undefined);
              }
            }}
            options={spaces.map((s) => ({ value: s.id, label: s.name }))}
          />
          <Select
            style={{ minWidth: 180 }}
            allowClear
            showSearch
            optionFilterProp="label"
            placeholder="不挂资源集"
            value={collectionId}
            onChange={(v) => {
              setCollectionId(v);
              // 选中资源集 → 空间自动跟随
              if (v) {
                const col = collections.find((c) => c.id === v);
                if (col) setSpaceId(col.spaceId);
              }
            }}
            options={spaces
              .map((s) => ({
                label: s.name,
                options: collections
                  .filter((c) => c.spaceId === s.id)
                  .map((c) => ({ value: c.id, label: c.name })),
              }))
              .filter((g) => g.options.length > 0)}
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
