import { Component, type ErrorInfo, type ReactNode } from "react";
import { Button, Result } from "antd";

/**
 * 页面级错误边界 — 防止单个页面渲染崩溃导致整个应用白屏。
 *
 * 背景：收件箱详情曾因前后端契约漂移（preview.lines 为 undefined 时调 .join）
 * 在 render 期抛 TypeError，无 ErrorBoundary 时 React 卸载整棵树 → 全白屏。
 * 在 AppShell 内容区套上边界后：崩溃只影响页面区域，侧边栏导航仍可用，
 * 用户可切页或点「重新加载」恢复。
 */

interface ErrorBoundaryProps {
  children: ReactNode;
}

interface ErrorBoundaryState {
  error: Error | null;
}

export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  state: ErrorBoundaryState = { error: null };

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    // 保留完整堆栈到控制台，便于排查（Tauri 下可在 devtools 查看）
    console.error("[ErrorBoundary] 页面渲染崩溃：", error, info.componentStack);
  }

  render() {
    const { error } = this.state;
    if (error) {
      return (
        <Result
          status="error"
          title="页面出错了"
          subTitle={error.message || String(error)}
          extra={[
            <Button
              key="retry"
              onClick={() => this.setState({ error: null })}
            >
              重试
            </Button>,
            <Button key="reload" type="primary" onClick={() => window.location.reload()}>
              重新加载
            </Button>,
          ]}
        />
      );
    }
    return this.props.children;
  }
}
