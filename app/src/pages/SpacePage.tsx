import { useState, useEffect, useCallback } from "react";
import type { Space, ApiError } from "../api";
import { spaceList, spaceArchive, spaceRestore, toApiError } from "../api";
import { SpaceDialog } from "../components/SpaceDialog";
import { ConfirmDialog } from "../components/ConfirmDialog";
import "./SpacePage.css";

type StatusFilter = "active" | "archived";

export function SpacePage() {
  const [spaces, setSpaces] = useState<Space[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<ApiError | null>(null);
  const [statusFilter, setStatusFilter] = useState<StatusFilter>("active");

  // Dialog state
  const [dialogOpen, setDialogOpen] = useState(false);
  const [editingSpace, setEditingSpace] = useState<Space | undefined>(undefined);
  const [archiveTarget, setArchiveTarget] = useState<Space | null>(null);

  const fetchSpaces = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await spaceList({ status: statusFilter });
      setSpaces(result);
    } catch (err) {
      setError(toApiError(err));
    } finally {
      setLoading(false);
    }
  }, [statusFilter]);

  useEffect(() => {
    fetchSpaces();
  }, [fetchSpaces]);

  function handleCreate() {
    setEditingSpace(undefined);
    setDialogOpen(true);
  }

  function handleEdit(space: Space) {
    setEditingSpace(space);
    setDialogOpen(true);
  }

  function handleArchiveClick(space: Space) {
    setArchiveTarget(space);
  }

  async function handleArchiveConfirm() {
    if (!archiveTarget) return;
    try {
      await spaceArchive({ id: archiveTarget.id });
      setArchiveTarget(null);
      fetchSpaces();
    } catch (err) {
      setError(toApiError(err));
      setArchiveTarget(null);
    }
  }

  async function handleRestore(space: Space) {
    try {
      await spaceRestore({ id: space.id });
      fetchSpaces();
    } catch (err) {
      setError(toApiError(err));
    }
  }

  return (
    <div className="space-page">
      <div className="space-page-header">
        <h1>空间</h1>
        <div className="space-page-actions">
          <div className="space-filter">
            <button
              className={`space-filter-btn ${statusFilter === "active" ? "active" : ""}`}
              onClick={() => setStatusFilter("active")}
            >
              活跃
            </button>
            <button
              className={`space-filter-btn ${statusFilter === "archived" ? "active" : ""}`}
              onClick={() => setStatusFilter("archived")}
            >
              已归档
            </button>
          </div>
          <button className="space-btn-create" onClick={handleCreate}>
            + 创建空间
          </button>
        </div>
      </div>

      {error && (
        <div className="space-error">
          <span>{error.message}</span>
          {error.retryable && (
            <button className="space-error-retry" onClick={fetchSpaces}>
              重试
            </button>
          )}
        </div>
      )}

      {loading ? (
        <div className="space-loading">加载中...</div>
      ) : spaces.length === 0 ? (
        <div className="space-empty">
          {statusFilter === "active" ? "暂无活跃空间，点击上方按钮创建。" : "暂无已归档空间。"}
        </div>
      ) : (
        <div className="space-list">
          {spaces.map((space) => (
            <div key={space.id} className="space-card">
              <div className="space-card-header">
                <span
                  className="space-card-color"
                  style={{ backgroundColor: space.color || "var(--color-accent)" }}
                />
                <span className="space-card-name">{space.name}</span>
                <span className={`space-card-status ${space.status}`}>
                  {space.status === "active" ? "活跃" : "已归档"}
                </span>
              </div>
              {space.description && (
                <p className="space-card-desc">{space.description}</p>
              )}
              <div className="space-card-meta">
                <span>创建于 {new Date(space.createdAt).toLocaleDateString()}</span>
              </div>
              <div className="space-card-actions">
                {space.status === "active" ? (
                  <>
                    <button className="space-btn space-btn-edit" onClick={() => handleEdit(space)}>
                      编辑
                    </button>
                    <button className="space-btn space-btn-archive" onClick={() => handleArchiveClick(space)}>
                      归档
                    </button>
                  </>
                ) : (
                  <button className="space-btn space-btn-restore" onClick={() => handleRestore(space)}>
                    恢复
                  </button>
                )}
              </div>
            </div>
          ))}
        </div>
      )}

      <SpaceDialog
        open={dialogOpen}
        space={editingSpace}
        onClose={() => setDialogOpen(false)}
        onSaved={fetchSpaces}
      />

      <ConfirmDialog
        open={!!archiveTarget}
        title="归档空间"
        message={`确定要归档空间"${archiveTarget?.name}"吗？归档后不可编辑，但可随时恢复。`}
        confirmLabel="归档"
        danger
        onConfirm={handleArchiveConfirm}
        onCancel={() => setArchiveTarget(null)}
      />
    </div>
  );
}
