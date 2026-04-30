# 需求文档：多目标同时 Ping

## 简介

当前 PingPlotter Next 的 Ping 监控页面仅支持同时监控单个目标。本功能扩展 Ping 页面，使其支持以标签页（Tab）形式同时监控多个目标（如 8.8.8.8、google.com、baidu.com 等），每个目标拥有独立的配置、图表、统计和结果表格。后端 ICMP 服务已支持多 session 并发管理，本功能的核心工作集中在前端 UI 层面。

## 术语表

- **Ping_View**：Ping 监控主页面组件（`PingView.vue`），承载所有目标标签页的容器
- **Tab_Bar**：标签栏组件，显示所有已添加目标的标签页，支持切换、新增和关闭操作
- **Target_Tab**：单个目标标签页，包含该目标的配置面板、延迟图表、统计面板和结果表格
- **Ping_Store**：Pinia 状态管理（`pingStore.ts`），管理所有目标的配置、结果和运行状态
- **ICMP_Service**：后端 Rust ICMP 服务（`icmp.rs`），负责执行实际的 Ping 操作
- **Ping_Config**：配置面板组件（`PingConfig.vue`），用于设置目标地址和 Ping 参数
- **Active_Tab**：当前用户正在查看的标签页
- **Target_List**：所有已添加的 Ping 目标集合

## 需求

### 需求 1：多目标标签页管理

**用户故事：** 作为网络管理员，我希望能以标签页形式同时管理多个 Ping 目标，以便在一个页面内监控多个网络节点的连通性。

#### 验收标准

1. WHEN 用户点击标签栏中的"添加"按钮，THE Ping_View SHALL 创建一个新的 Target_Tab 并将其设为 Active_Tab
2. THE Ping_View SHALL 在页面顶部显示 Tab_Bar，其中包含所有已添加目标的标签页
3. WHEN 用户点击某个 Target_Tab，THE Ping_View SHALL 将该标签页切换为 Active_Tab 并显示其对应的监控内容
4. WHEN 用户点击 Target_Tab 上的关闭按钮，THE Ping_View SHALL 停止该目标的 Ping 会话并从 Target_List 中移除该目标
5. IF 用户关闭最后一个 Target_Tab，THEN THE Ping_View SHALL 自动创建一个新的空白 Target_Tab
6. THE Tab_Bar SHALL 在每个标签页上显示目标地址文本，对于正在运行的目标额外显示运行状态指示器
7. WHEN Ping_View 首次加载，THE Ping_View SHALL 创建一个包含默认目标地址"8.8.8.8"的 Target_Tab

### 需求 2：标签页内独立监控

**用户故事：** 作为网络管理员，我希望每个标签页拥有独立的配置、图表和统计数据，以便互不干扰地监控各个目标。

#### 验收标准

1. THE Target_Tab SHALL 包含独立的 Ping_Config 面板、延迟图表、统计面板和结果表格
2. WHEN 用户在某个 Target_Tab 中点击"开始 Ping"，THE ICMP_Service SHALL 仅启动该目标的 Ping 会话，其他标签页的状态保持不变
3. WHEN 用户在某个 Target_Tab 中点击"停止 Ping"，THE ICMP_Service SHALL 仅停止该目标的 Ping 会话，其他标签页的状态保持不变
4. WHEN 用户在某个 Target_Tab 中点击"清除结果"，THE Ping_Store SHALL 仅清除该目标的结果数据，其他标签页的数据保持不变
5. WHILE 某个 Target_Tab 不是 Active_Tab，THE Ping_Store SHALL 继续接收并存储该目标的 Ping 结果数据
6. WHEN 用户切换回某个后台运行的 Target_Tab，THE Target_Tab SHALL 显示该目标在后台期间累积的所有结果数据

### 需求 3：多目标并发运行

**用户故事：** 作为网络管理员，我希望能同时运行多个目标的 Ping 会话，以便实时对比不同节点的网络质量。

#### 验收标准

1. THE Ping_View SHALL 支持同时运行至少 10 个目标的 Ping 会话
2. WHEN 多个目标同时运行时，THE ICMP_Service SHALL 为每个目标维护独立的 Ping 循环和结果流
3. WHEN 后端发送 Ping 结果事件时，THE Ping_Store SHALL 根据结果中的 target 字段将数据路由到对应目标的存储中
4. WHEN 用户离开 Ping_View 页面（路由导航），THE Ping_View SHALL 停止所有正在运行的 Ping 会话并重置状态

### 需求 4：标签页状态可视化

**用户故事：** 作为网络管理员，我希望在标签栏上快速了解每个目标的运行状态，以便无需切换标签页即可掌握整体监控情况。

#### 验收标准

1. WHILE 某个目标的 Ping 会话正在运行，THE Tab_Bar SHALL 在该目标的标签页上显示绿色运行状态指示器
2. WHILE 某个目标的 Ping 会话已停止，THE Tab_Bar SHALL 在该目标的标签页上显示灰色停止状态指示器
3. WHEN 某个目标的最近一次 Ping 结果为超时，THE Tab_Bar SHALL 在该目标的标签页上显示红色警告状态指示器
4. THE Tab_Bar SHALL 在每个标签页上显示该目标的最新延迟值（毫秒），超时时显示"超时"文本

### 需求 5：标签页目标地址修改

**用户故事：** 作为网络管理员，我希望能在标签页内修改目标地址并重新开始 Ping，以便灵活调整监控目标。

#### 验收标准

1. WHILE 某个 Target_Tab 的 Ping 会话未运行，THE Ping_Config SHALL 允许用户修改目标地址输入框
2. WHEN 用户修改目标地址并点击"开始 Ping"，THE Ping_View SHALL 使用新的目标地址启动 Ping 会话，并更新 Tab_Bar 中该标签页的显示文本
3. WHILE 某个 Target_Tab 的 Ping 会话正在运行，THE Ping_Config SHALL 禁用目标地址输入框，防止运行中修改

### 需求 6：国际化支持

**用户故事：** 作为用户，我希望多目标 Ping 功能的所有新增 UI 文本都支持中英文切换，以便与现有的多语言体验保持一致。

#### 验收标准

1. THE Ping_View SHALL 将所有新增的 UI 文本（标签页操作提示、状态文本等）注册到 i18n 语言文件中
2. THE Ping_View SHALL 同时提供中文（zh）和英文（en）的翻译文本
3. WHEN 用户切换应用语言时，THE Ping_View SHALL 立即更新所有多目标相关的 UI 文本为对应语言

### 需求 7：全部停止操作

**用户故事：** 作为网络管理员，我希望能一键停止所有正在运行的 Ping 会话，以便快速释放系统资源。

#### 验收标准

1. WHEN 存在至少一个正在运行的 Ping 会话时，THE Ping_View SHALL 在页面中显示"全部停止"按钮
2. WHEN 用户点击"全部停止"按钮，THE Ping_View SHALL 停止所有正在运行的 Ping 会话并更新所有标签页的状态
3. WHEN 没有正在运行的 Ping 会话时，THE Ping_View SHALL 隐藏或禁用"全部停止"按钮
