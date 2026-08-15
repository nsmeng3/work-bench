import { useState, useEffect } from "react";
import type { Space, SpaceCreateInput, SpaceUpdateInput, ApiError } from "../api";
import { spaceCreate, spaceUpdate, toApiError } from "../api";
import "./SpaceDialog.css";

const NAME_MAX_LENGTH = 64;

interface SpaceDialogProps {
  open: boolean;
  /** 传入则为编辑模式，否则为创建模式 */
  space?: Space;
  onClose: () => void;
  onSaved: () => void;
}

export function SpaceDialog({ open, space, onClose, onSaved }: SpaceDialogProps) {
  const isEdit = !!space;
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [color, setColor] = useState("");
  const [icon, setIcon] = useState("");
  const [error, setError] = useState<ApiError | null>(null);
  const [submitting, setSubmitting] = useState(false);

  useEffect(() => {
    if (open) {
      setName(space?.name ?? "");
      setDescription(space?.description ?? "");
      setColor(space?.color ?? "");
      setIcon(space?.icon ?? "");
      setError(null);
      setSubmitting(false);
    }
  }, [open, space]);

  if (!open) return null;

  const nameTrimmed = name.trim();
  const nameTooLong = nameTrimmed.length > NAME_MAX_LENGTH;
  const nameValid = nameTrimmed.length > 0 && !nameTooLong;

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!nameValid || submitting) return;

    setSubmitting(true);
    setError(null);

    try {
      if (isEdit) {
        const input: SpaceUpdateInput = {
          id: space.id,
          name: nameTrimmed,
          description: description.trim() || undefined,
          color: color || undefined,
          icon: icon || undefined,
        };
        await spaceUpdate(input);
      } else {
        const input: SpaceCreateInput = {
          name: nameTrimmed,
          description: description.trim() || undefined,
          color: color || undefined,
          icon: icon || undefined,
        };
        await spaceCreate(input);
      }
      onSaved();
      onClose();
    } catch (err) {
      setError(toApiError(err));
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <div className="dialog-overlay" onClick={onClose}>
      <div className="dialog" onClick={(e) => e.stopPropagation()}>
        <h2 className="dialog-title">{isEdit ? "编辑空间" : "创建空间"}</h2>

        <form onSubmit={handleSubmit} className="dialog-form">
          <label className="dialog-field">
            <span className="dialog-label">名称 *</span>
            <input
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="空间名称"
              maxLength={NAME_MAX_LENGTH + 10}
              autoFocus
            />
            {nameTooLong && (
              <span className="dialog-field-error">名称超长（最大 {NAME_MAX_LENGTH} 字符）</span>
            )}
            {nameTrimmed.length === 0 && name.length > 0 && (
              <span className="dialog-field-error">名称不能为空</span>
            )}
          </label>

          <label className="dialog-field">
            <span className="dialog-label">描述</span>
            <textarea
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder="可选描述"
              rows={3}
            />
          </label>

          <label className="dialog-field">
            <span className="dialog-label">颜色</span>
            <input
              value={color}
              onChange={(e) => setColor(e.target.value)}
              placeholder="如 #4A90D9"
            />
          </label>

          <label className="dialog-field">
            <span className="dialog-label">图标</span>
            <input
              value={icon}
              onChange={(e) => setIcon(e.target.value)}
              placeholder="图标 key"
            />
          </label>

          {error && (
            <div className="dialog-error">
              <span>{error.message}</span>
              {error.retryable && (
                <button type="button" className="dialog-retry-btn" onClick={handleSubmit}>
                  重试
                </button>
              )}
            </div>
          )}

          <div className="dialog-actions">
            <button type="button" className="dialog-btn-cancel" onClick={onClose}>
              取消
            </button>
            <button type="submit" className="dialog-btn-primary" disabled={!nameValid || submitting}>
              {submitting ? "保存中..." : isEdit ? "保存" : "创建"}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
