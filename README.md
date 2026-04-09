# PingPlotter Next

> 网络监控桌面应用 - 类似 PingPlotter，提供 Ping、路由追踪、带宽测试功能

![License](https://img.shields.io/badge/license-MIT-blue.svg)
![Tauri](https://img.shields.io/badge/Tauri-2.0-blue.svg)
![Vue](https://img.shields.io/badge/Vue-3.4-green.svg)
![Rust](https://img.shields.io/badge/Rust-2021-orange.svg)

## 功能特性

- **实时 Ping 监控** - 持续监控目标主机延迟和丢包率
- **路由追踪** - 追踪数据包经过的网络节点
- **带宽测试** - 测试网络上传和下载速度
- **历史记录** - 查看历史 Ping 会话数据
- **暗黑主题** - 舒适的深色界面

## 技术栈

### 前端
| 技术 | 说明 |
|------|------|
| Vue 3 | 渐进式 JavaScript 框架 (Composition API) |
| TypeScript | 类型安全的 JavaScript 超集 |
| Pinia | Vue 3 推荐的状态管理 |
| Vue Router | 官方路由管理器 |
| ECharts | 强大的数据可视化图表库 |
| Vite | 下一代前端构建工具 |
| Sass | CSS 预处理器 |

### 后端 (Tauri)
| 依赖 | 说明 |
|------|------|
| Tauri 2 | 跨平台桌面应用框架 |
| Tokio | 异步运行时 |
| surge-ping | ICMP Ping 实现 |
| trust-dns-resolver | DNS 解析 |
| rusqlite | SQLite 数据库 |
| serde/serde_json | 序列化/反序列化 |
| chrono | 日期时间处理 |

## 项目结构

```
PingPlotter-Next/
├── src/                      # 前端源码
│   ├── views/                # 页面视图
│   │   ├── Dashboard.vue     # 仪表盘
│   │   ├── PingView.vue      # Ping 监控
│   │   ├── TraceView.vue     # 路由追踪
│   │   ├── BandwidthView.vue # 带宽测试
│   │   └── HistoryView.vue   # 历史记录
│   ├── components/           # 组件
│   │   ├── common/           # 通用组件
│   │   ├── ping/             # Ping 相关组件
│   │   ├── traceroute/       # 路由追踪组件
│   │   └── bandwidth/        # 带宽测试组件
│   ├── composables/          # 组合式函数 (与后端交互)
│   ├── stores/               # Pinia 状态管理
│   ├── router/               # 路由配置
│   ├── types/                # TypeScript 类型定义
│   └── utils/                # 工具函数
├── src-tauri/                # Tauri 后端源码
│   ├── src/
│   │   ├── commands/         # Tauri 命令 (前端 API)
│   │   ├── services/         # 业务逻辑层
│   │   ├── models/           # 数据模型
│   │   └── storage/          # 数据存储
│   ├── icons/                # 应用图标
│   ├── Cargo.toml            # Rust 依赖配置
│   └── tauri.conf.json       # Tauri 配置
├── package.json              # 前端依赖配置
└── README.md                 # 项目文档
```

## 开发指南

### 环境要求

- [Node.js](https://nodejs.org/) v18+
- [Rust](https://www.rust-lang.org/tools/install) v1.70+
- Windows 10/11 (开发环境)

### 安装依赖

```bash
npm install
```

### 开发模式

启动开发服务器（自动热重载）：

```bash
npm run tauri dev
```

### 构建发布

构建生产版本：

```bash
npm run tauri build
```

构建产物位于 `src-tauri/target/release/bundle/`

### 可用命令

| 命令 | 说明 |
|------|------|
| `npm run dev` | 启动前端开发服务器 |
| `npm run build` | 构建前端 |
| `npm run preview` | 预览构建结果 |
| `npm run tauri dev` | 启动完整的 Tauri 开发环境 |
| `npm run tauri build` | 构建完整的桌面应用 |

## 使用说明

### Ping 监控

1. 在目标输入框中输入 IP 地址或域名（如 `8.8.8.8` 或 `google.com`）
2. 配置参数：
   - **Interval** - Ping 间隔（毫秒）
   - **Timeout** - 超时时间（毫秒）
   - **Packet Size** - 数据包大小（字节）
3. 点击 **Start Ping** 开始监控
4. 实时查看延迟图表和统计数据
5. 点击 **Clear Results** 清空当前结果

### 路由追踪

1. 输入目标地址
2. 点击 **Start Trace**
3. 查看路由跳数、延迟和地理位置信息

### 带宽测试

1. 选择测试服务器
2. 点击 **Start Test**
3. 查看上传/下载速度

## 数据存储

- Ping 会话数据存储在 SQLite 数据库中
- 每个目标最多保留最近 1000 条结果在内存中
- 会话结束或应用退出时自动保存到数据库

## 内存优化

已实施以下内存优化措施：

1. **后端限制** - 每个 Ping 会话在内存中最多保留 1000 条结果
2. **前端限制** - 图表组件只显示最近 10 个数据点
3. **事件监听** - 使用单例模式避免重复注册事件监听器
4. **数组优化** - 避免不必要的数组复制

## 常见问题

### Q: Ping 命令失败 "拒绝访问"
A: 某些网络环境可能需要管理员权限运行 ICMP Ping。尝试以管理员身份运行应用。

### Q: DNS 解析失败
A: 应用会先尝试系统 DNS，失败后自动切换到公共 DNS（1.1.1.1, 8.8.8.8, 223.5.5.5）。

### Q: 应用卡顿或内存占用高
A: 长时间运行的 Ping 会话会积累数据。点击 "Clear Results" 或重启会话可释放内存。

## 许可证

MIT License

## 致谢

- [Tauri](https://tauri.app/) - 跨平台桌面应用框架
- [Vue.js](https://vuejs.org/) - 渐进式 JavaScript 框架
- [ECharts](https://echarts.apache.org/) - 数据可视化库
- [surge-ping](https://github.com/surge-networks/surge-ping) - Rust Ping 实现

---

**Built with ❤️ using Tauri + Vue + Rust**
