# RouteDoctor

> 开源网络监控桌面应用 - 提供持续路径监控、Ping 监控、带宽测试、DNS 查询等功能

![License](https://img.shields.io/badge/license-MIT-blue.svg)
![Tauri](https://img.shields.io/badge/Tauri-2.0-blue.svg)
![Vue](https://img.shields.io/badge/Vue-3.4-green.svg)
![Rust](https://img.shields.io/badge/Rust-2021-orange.svg)

## 功能特性

### 核心功能

- **🗺️ 持续路径监控** - 发现网络路径后对每一跳持续 Ping，通过延迟热力图实时发现网络瓶颈
- **📡 Ping 监控** - 多标签页同时监控多个目标，实时延迟图表、统计数据、丢包率
- **⚡ 带宽测试** - 测量下载/上传速度，动态量程仪表盘
- **🔍 DNS 查询** - 支持 A/AAAA/CNAME/MX/NS/TXT 等多种记录类型
- **🖥️ 网络信息** - 本机 IP、公网 IP、DNS 服务器、网络接口一览

### 进阶功能

- **⚠️ 告警系统** - 延迟/丢包/连续超时阈值告警，Toast + 系统通知
- **★ 目标收藏夹** - 保存常用监控目标，快速切换
- **⬇ 数据导出** - 支持 CSV/JSON 格式导出 Ping、Traceroute、带宽测试数据
- **📊 历史记录** - 查看历史测试结果详情，支持筛选和搜索
- **🌙 主题切换** - 深色/浅色/跟随系统，平滑过渡动画
- **🌐 国际化** - 中文/英文双语支持

### 用户体验

- 侧边栏可折叠，适配不同屏幕尺寸
- Toast 通知系统，操作反馈即时可见
- 骨架屏加载状态，减少等待焦虑
- Esc 键关闭弹窗，Enter 键快速操作
- 响应式布局，窗口缩放无水平溢出

## 技术栈

### 前端
| 技术 | 说明 |
|------|------|
| Vue 3 | 渐进式 JavaScript 框架 (Composition API) |
| TypeScript | 类型安全的 JavaScript 超集 |
| Pinia | Vue 3 状态管理 |
| Vue Router | 路由管理（keep-alive 缓存） |
| ECharts | 数据可视化（折线图、仪表盘、热力图） |
| vue-i18n | 国际化 |
| Vite | 构建工具 |
| Sass | CSS 预处理器 |

### 后端 (Tauri/Rust)
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
RouteDoctor/
├── src/                          # 前端源码
│   ├── views/                    # 页面视图
│   │   ├── Dashboard.vue         # 仪表盘（实时状态概览）
│   │   ├── PingView.vue          # Ping 监控（多标签页）
│   │   ├── TraceView.vue         # 路径监控（热力图 + 统计）
│   │   ├── BandwidthView.vue     # 带宽测试（动态仪表盘）
│   │   ├── DnsView.vue           # DNS 查询
│   │   ├── NetworkInfoView.vue   # 网络信息（骨架屏加载）
│   │   ├── HistoryView.vue       # 历史记录（详情弹窗）
│   │   └── SettingsView.vue      # 设置
│   ├── components/               # 组件
│   │   ├── common/               # 通用组件（Toast、侧边栏、确认框）
│   │   ├── ping/                 # Ping 组件（图表、统计、表格、标签栏）
│   │   ├── bandwidth/            # 带宽组件（仪表盘、配置）
│   │   └── history/              # 历史组件（详情弹窗）
│   ├── composables/              # 组合式函数
│   │   ├── usePing.ts            # Ping 后端交互
│   │   ├── useContinuousTrace.ts # 持续路径监控
│   │   ├── useTraceroute.ts      # Traceroute 后端交互
│   │   ├── useBandwidth.ts       # 带宽测试
│   │   ├── useToast.ts           # Toast 通知系统
│   │   └── useExport.ts          # 数据导出（CSV/JSON）
│   ├── stores/                   # Pinia 状态管理
│   │   ├── pingStore.ts          # Ping 数据
│   │   ├── continuousTraceStore.ts # 持续路径监控数据
│   │   ├── bandwidthStore.ts     # 带宽测试数据
│   │   ├── historyStore.ts       # 历史记录
│   │   ├── alertStore.ts         # 告警规则和事件
│   │   ├── favoritesStore.ts     # 目标收藏夹
│   │   └── settingsStore.ts      # 应用设置
│   ├── i18n/                     # 国际化
│   ├── router/                   # 路由配置
│   └── types/                    # TypeScript 类型定义
├── src-tauri/                    # Tauri 后端源码
│   └── src/
│       ├── commands/             # Tauri 命令
│       │   ├── ping.rs           # Ping 命令
│       │   ├── continuous_trace.rs # 持续路径监控
│       │   ├── traceroute.rs     # 单次 Traceroute
│       │   ├── bandwidth.rs      # 带宽测试
│       │   ├── network.rs        # 网络信息/DNS
│       │   └── history.rs        # 历史记录
│       ├── services/             # 业务逻辑层
│       │   ├── icmp.rs           # ICMP Ping 服务
│       │   ├── continuous_trace.rs # 持续监控服务
│       │   ├── traceroute.rs     # Traceroute 服务
│       │   ├── bandwidth.rs      # 带宽测试服务
│       │   └── dns.rs            # DNS 解析服务
│       ├── models/               # 数据模型
│       └── storage/              # SQLite 数据存储
├── package.json
└── README.md
```

## 开发指南

### 环境要求

- [Node.js](https://nodejs.org/) v18+
- [Rust](https://www.rust-lang.org/tools/install) v1.70+
- Windows 10/11（主要开发平台）

### 安装依赖

```bash
npm install
```

### 开发模式

推荐使用包含 Npcap SDK 自动准备的命令：

```bash
npm run tauri:dev      # 自动准备 Npcap SDK + 启动开发模式
```

或传统命令（如果你已经手动配好 SDK）：

```bash
npm run tauri dev
```

### 构建发布

```bash
npm run tauri:build    # 自动准备 Npcap SDK + 打包桌面应用
```

构建产物位于 `src-tauri/target/release/bundle/`

### Npcap 集成说明

应用在 Windows 上集成 Npcap 以获取真实 UDP/TCP 路径追踪能力：

- **最终用户**：只需要安装 [Npcap 驱动](https://npcap.com/#download)（运行时）。未装时应用自动回退到基础模式
- **开发者**：构建期需要 Npcap SDK 中的 `wpcap.lib`。`npm run tauri:dev` / `tauri:build` 会自动调用 `scripts/setup-npcap-sdk.mjs` 下载到 `src-tauri/.npcap-sdk/`
- **CI 构建机**：参考 `.github/workflows/build-windows.yml.example`
- **macOS / Linux 构建**：脚本自动跳过，pcap crate 链接系统的 libpcap

如果自动下载失败（网络受限），可以手动运行：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File src-tauri/scripts/setup-npcap-sdk.ps1
```

或设置环境变量指向已有 SDK：

```powershell
$env:NPCAP_SDK_DIR = "C:\path\to\Npcap-SDK"
```

### 可用命令

| 命令 | 说明 |
|------|------|
| `npm run dev` | 启动前端开发服务器 |
| `npm run build` | 构建前端（vue-tsc + vite） |
| `npm run tauri dev` | 启动完整开发环境（不预装 SDK） |
| `npm run tauri build` | 构建桌面应用（不预装 SDK） |
| `npm run tauri:dev` | 启动开发环境（自动准备 Npcap SDK） |
| `npm run tauri:build` | 构建桌面应用（自动准备 Npcap SDK） |
| `npm run setup:npcap-sdk` | 单独触发 Npcap SDK 下载 |

## 使用说明

### 持续路径监控

1. 进入 **Traceroute** 页面
2. 输入目标地址，选择探测方式（ICMP/UDP/TCP）
3. 点击 **开始监控**
4. 等待路径发现完成（几秒钟）
5. 查看延迟热力图 — 横轴时间，纵轴跳数，颜色表示延迟
6. 下方统计表格显示每跳的平均/最小/最大延迟和丢包率

### Ping 监控

1. 输入目标地址，配置间隔/超时/包大小
2. 点击 **开始 Ping** 或按 Enter
3. 支持多标签页同时监控多个目标
4. 切换页面后 Ping 在后台继续运行
5. 点击 ⬇ 按钮导出数据为 CSV
6. 点击 ★ 按钮收藏目标

### 带宽测试

1. 点击 **开始测速**
2. 自动测试下载和上传速度
3. 仪表盘动态量程自适应（10Mbps ~ 10Gbps）
4. 测试完成后 Toast 通知结果

### 告警系统

默认告警规则（可在代码中自定义）：
- 延迟 > 200ms 连续 3 次 → Toast 告警
- 连续超时 5 次 → Toast + 系统通知

## 数据存储

- 测试结果存储在 SQLite 数据库中
- 应用设置和收藏夹存储在 localStorage
- 每个 Ping 目标最多保留 1000 条结果在内存中
- 应用退出时自动保存未完成的会话

## 许可证

MIT License

## 致谢

- [Tauri](https://tauri.app/) - 跨平台桌面应用框架
- [Vue.js](https://vuejs.org/) - 渐进式 JavaScript 框架
- [ECharts](https://echarts.apache.org/) - 数据可视化库
- [surge-ping](https://github.com/surge-networks/surge-ping) - Rust Ping 实现

---

**Built with ❤️ using Tauri + Vue + Rust**
