import { useEffect, useState } from "react";
import { Button, Space as AntSpace, Tooltip } from "antd";
import {
  DeleteOutlined,
  InboxOutlined,
  RollbackOutlined,
  WarningOutlined,
} from "@ant-design/icons";
import type { DispCapabilities, Reference } from "../api";
import { dispGetCapabilities, toApiError } from "../api";

/**
 * 三档处置按钮组 — 详细设计 §2.6 / §4.2 / §6.7。
 *
 * 视觉分级（antd v5）：
 * - 归档 / 恢复：`Button type="default"`（蓝色系）
 * - 删除（回收站）：`Button danger`（黄色预警）
 * - 销毁：`Button type="primary" danger`（红色高危）
 *
 * 可用性：按 `disp_get_capabilities` 返回渲染；不可用项 `disabled` + tooltip 显示 `reason`。
 */

export type DispositionAction = "archive" | "unarchive" | "softDelete" | "destroy";

interface DispositionButtonsProps {
  /** 目标引用 */
  reference: Reference;
  /** 用户点击某个动作时触发；父组件负责弹确认框 / 调命令 / 刷新 */
  onAction: (action: DispositionAction) => void;
  /** 按钮尺寸，默认 "small" */
  size?: "small" | "middle" | "large";
}

export function DispositionButtons({
  reference,
  onAction,
  size = "small",
}: DispositionButtonsProps) {
  const [caps, setCaps] = useState<DispCapabilities | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setCaps(null);
    setLoadError(null);
    dispGetCapabilities(reference.id)
      .then((c) => {
        if (!cancelled) setCaps(c);
      })
      .catch((err) => {
        if (!cancelled) setLoadError(toApiError(err).message);
      });
    return () => {
      cancelled = true;
    };
  }, [reference.id, reference.disposition]);

  if (loadError) {
    return (
      <Tooltip title={`加载处置能力失败：${loadError}`}>
        <Button size={size} type="text" disabled>
          处置不可用
        </Button>
      </Tooltip>
    );
  }

  if (!caps) {
    return (
      <AntSpace size={4}>
        <Button size={size} type="text" loading />
      </AntSpace>
    );
  }

  // 归档 / 恢复：根据 disposition 二选一展示
  const isArchived = reference.disposition === "archived";
  const archiveBtn = isArchived ? (
    <Tooltip key="unarchive" title={caps.archive ? "恢复引用到主列表" : undefined}>
      <Button
        size={size}
        type="default"
        icon={<RollbackOutlined />}
        onClick={() => onAction("unarchive")}
      >
        恢复
      </Button>
    </Tooltip>
  ) : (
    <Tooltip key="archive" title={caps.archive ? undefined : caps.reason.archive}>
      <Button
        size={size}
        type="default"
        icon={<InboxOutlined />}
        disabled={!caps.archive}
        onClick={() => onAction("archive")}
      >
        归档
      </Button>
    </Tooltip>
  );

  const deleteBtn = (
    <Tooltip key="softDelete" title={caps.softDelete ? "移入系统回收站" : caps.reason.softDelete}>
      <Button
        size={size}
        danger
        icon={<DeleteOutlined />}
        disabled={!caps.softDelete}
        onClick={() => onAction("softDelete")}
      >
        删除
      </Button>
    </Tooltip>
  );

  const destroyBtn = (
    <Tooltip key="destroy" title={caps.destroy ? "永久删除（不可恢复）" : caps.reason.destroy}>
      <Button
        size={size}
        type="primary"
        danger
        icon={<WarningOutlined />}
        disabled={!caps.destroy}
        onClick={() => onAction("destroy")}
      >
        销毁
      </Button>
    </Tooltip>
  );

  return (
    <AntSpace size={4} wrap>
      {archiveBtn}
      {deleteBtn}
      {destroyBtn}
    </AntSpace>
  );
}
