# 实现计划：多目标同时 Ping

## 概述

将 PingView 从单目标监控升级为多标签页多目标并发监控。核心改动集中在前端 UI 层：新增 PingTabBar 组件、重构 PingView 为多标签页编排层、pingStore 新增 getter、新增类型定义和 i18n 键值。后端和现有子组件（PingConfig、PingChart、PingStats、PingTable）无需改动。

## 任务

- [x] 1. 新增类型定义和 pingStore getter
  - [x] 1.1 在 `src/types/index.ts` 中新增 `PingTab` 接口
    - 定义 `id: string`（唯一标识符）和 `target: string`（目标地址）字段
    - _需求: 1.1, 1.2_
  - [x] 1.2 在 `src/stores/pingStore.ts` 中新增 `getLatestResult` getter
    - 实现 `getLatestResult(target: string): PingResult | undefined`，返回指定目标的最新一条结果
    - 将该 getter 添加到 store 的 return 对象中导出
    - _需求: 4.3, 4.4_

- [x] 2. 新增 i18n 国际化键值
  - [x] 2.1 在 `src/i18n/locales/zh.ts` 的 `ping` 命名空间下新增中文键值
    - 新增键值：`addTab`（添加目标）、`closeTab`（关闭标签页）、`stopAll`（全部停止）、`newTab`（新标签页）、`defaultTarget`（8.8.8.8）、`tabTimeout`（超时）
    - _需求: 6.1, 6.2_
  - [x] 2.2 在 `src/i18n/locales/en.ts` 的 `ping` 命名空间下新增英文键值
    - 新增对应英文键值：`addTab`（Add Target）、`closeTab`（Close Tab）、`stopAll`（Stop All）、`newTab`（New Tab）、`defaultTarget`（8.8.8.8）、`tabTimeout`（Timeout）
    - _需求: 6.1, 6.2_

- [x] 3. 检查点 - 确保类型和 store 改动无编译错误
  - 确保所有改动通过编译，如有问题请询问用户。

- [x] 4. 新增 PingTabBar.vue 组件
  - [x] 4.1 创建 `src/components/ping/PingTabBar.vue` 组件
    - 实现 Props 接口：接收 `tabs: PingTab[]` 和 `activeTabId: string`
    - 实现 Emits：`select(tabId)`、`add()`、`close(tabId)`
    - 渲染标签页列表，每个标签显示目标地址文本（空目标显示 i18n `newTab` 文本）
    - 高亮当前活跃标签页（activeTabId 匹配）
    - 每个标签页显示运行状态指示器（绿色圆点=运行中、灰色圆点=已停止、红色圆点=超时）
    - 每个标签页显示最新延迟值（毫秒），超时时显示"超时"文本
    - 提供"+"按钮用于新增标签页
    - 每个标签页提供"×"关闭按钮
    - 标签页过多时支持横向滚动
    - 从 `pingStore` 通过 `isRunning()` 和 `getLatestResult()` 派生标签状态
    - _需求: 1.1, 1.2, 1.3, 1.6, 4.1, 4.2, 4.3, 4.4_
  - [ ]* 4.2 为 PingTabBar 编写单元测试
    - 测试标签页渲染、切换、新增、关闭等交互行为
    - 测试状态指示器根据运行状态正确显示
    - _需求: 1.1, 1.2, 1.3, 4.1, 4.2, 4.3_

- [x] 5. 重构 PingView.vue 为多标签页编排层
  - [x] 5.1 重构 `src/views/PingView.vue` 内部状态管理
    - 引入 `tabs: Ref<PingTab[]>` 和 `activeTabId: Ref<string>` 响应式状态
    - 实现 `activeTab` 和 `activeTarget` 计算属性
    - 实现 `hasRunningTargets` 计算属性（用于控制"全部停止"按钮显隐）
    - 页面首次加载（`onActivated` 或 `onMounted`）时创建一个默认标签页，目标为 `8.8.8.8`
    - _需求: 1.7, 3.1_
  - [x] 5.2 实现标签页管理方法
    - 实现 `addTab()`：生成唯一 tabId（`crypto.randomUUID()`），创建空白标签页，设为活跃
    - 实现 `closeTab(tabId)`：停止该目标 Ping 会话，从 store 移除数据，从 tabs 中移除；若关闭最后一个标签页，自动创建新的默认标签页
    - 实现 `selectTab(tabId)`：切换活跃标签页
    - _需求: 1.1, 1.3, 1.4, 1.5_
  - [x] 5.3 重构 Ping 操作处理方法
    - 重构 `handleStart(config)`：更新当前标签页的 target 字段，调用 store 和后端启动 Ping
    - 保持 `handleStop(target)` 和 `handleClear(target)` 逻辑不变
    - 实现 `stopAllRunning()`：遍历所有运行中的目标，逐一停止 Ping 会话并更新状态
    - _需求: 2.1, 2.2, 2.3, 2.4, 5.2, 7.2_
  - [x] 5.4 更新 PingView 模板，集成 PingTabBar 和"全部停止"按钮
    - 在页面顶部（标题下方）添加 PingTabBar 组件，绑定 tabs、activeTabId、事件处理
    - 将现有子组件（PingConfig、PingChart、PingStats、PingTable）的 `:target` prop 绑定到 `activeTarget`
    - PingConfig 的 `:is-running` 绑定到 `pingStore.isRunning(activeTarget)`
    - 添加"全部停止"按钮，仅在 `hasRunningTargets` 为 true 时显示
    - _需求: 1.2, 1.3, 2.1, 7.1, 7.3_
  - [x] 5.5 处理 keep-alive 生命周期
    - 使用 `onActivated` 替代或补充 `onMounted` 进行初始化（创建默认标签页）
    - 使用 `onDeactivated` 替代或补充 `onUnmounted` 进行清理（停止所有 Ping 会话、重置状态）
    - 确保从其他页面切换回来时标签页状态正确恢复
    - _需求: 3.4_

- [x] 6. 检查点 - 确保所有组件编译通过，功能可用
  - 确保所有改动通过编译，如有问题请询问用户。

- [x] 7. 集成验证与边界情况处理
  - [x] 7.1 验证多目标并发运行
    - 确保多个标签页可同时启动 Ping 会话，后台标签页持续接收数据
    - 确保切换标签页时显示该目标在后台累积的所有结果
    - 确保 `usePingListener` 正确将结果路由到对应目标的 store 中
    - _需求: 2.5, 2.6, 3.1, 3.2, 3.3_
  - [x] 7.2 验证标签页目标地址修改
    - 确保未运行时可修改目标地址输入框
    - 确保修改目标地址后点击"开始 Ping"能正确更新标签页显示文本和启动新会话
    - 确保运行中时目标地址输入框被禁用
    - _需求: 5.1, 5.2, 5.3_
  - [ ]* 7.3 为 PingView 多标签页逻辑编写单元测试
    - 测试 addTab、closeTab、selectTab 方法
    - 测试关闭最后一个标签页自动创建新标签页
    - 测试 stopAllRunning 方法
    - _需求: 1.1, 1.4, 1.5, 7.2_

- [x] 8. 最终检查点 - 确保所有测试通过
  - 确保所有改动通过编译和测试，如有问题请询问用户。

## 说明

- 标记 `*` 的子任务为可选任务，可跳过以加快 MVP 进度
- 每个任务引用了对应的需求编号，确保可追溯性
- 后端（Rust/Tauri）和现有子组件（PingConfig、PingChart、PingStats、PingTable）无需改动
- 检查点任务用于增量验证，确保每个阶段的改动正确
