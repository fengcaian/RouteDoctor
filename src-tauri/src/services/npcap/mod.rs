// Npcap 检测与集成模块
//
// 当前阶段（阶段 1）：仅做"是否安装"的检测。
// 后续阶段（阶段 2/3）将基于此模块在 Windows 上启用真正的 UDP/TCP traceroute
// （通过 Npcap 直接读写网卡，绕过 Windows 内核对 ICMP Time Exceeded 的过滤）。
//
// 法律说明：Npcap 免费版协议禁止重新分发，因此本应用不会内嵌 Npcap 安装包，
// 仅做"检测 + 引导用户去官网下载"。商业化时再考虑购买 OEM 许可。

pub mod detect;

// pcap 真实集成（仅 Windows）：用 Npcap 抓所有 ICMP 包，绕过 Windows 内核
// 对 UDP/TCP 触发的 ICMP Time Exceeded 的过滤，得到完整路径
#[cfg(windows)]
pub mod pcap_udp_traceroute;
#[cfg(windows)]
pub mod pcap_tcp_traceroute;
