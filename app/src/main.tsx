import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import FloatApp from "./float/FloatApp";

/** 悬浮通知窗（Rust 以 index.html?float=1 创建）渲染 FloatApp，其余窗口渲染主界面 */
const isFloatWindow = new URLSearchParams(window.location.search).has("float");

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>{isFloatWindow ? <FloatApp /> : <App />}</React.StrictMode>,
);
