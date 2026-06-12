// Npcap 安装状态检测
//
// 检测策略（按可靠性递减）：
// 1. 检查驱动文件是否存在（最可靠，所有 Npcap 版本一致）：
//    C:\Windows\System32\Npcap\wpcap.dll  —— 默认安装位置（Npcap 模式）
//    C:\Windows\System32\wpcap.dll        —— "WinPcap 兼容"安装位置
// 2. 读注册表确认版本号：
//    HKLM\SOFTWARE\WOW6432Node\Npcap (32 位视图，Npcap 安装器写到这里)
//    HKLM\SOFTWARE\Npcap            (64 位视图，老版本可能写到这里)
// 3. 检查驱动服务（npcap）是否在运行：
//    HKLM\SYSTEM\CurrentControlSet\Services\npcap
//
// 非 Windows 平台直接返回"未安装"，因为 Npcap 是 Windows 专属。

use serde::Serialize;

/// Npcap 安装状态。前端根据此状态决定 UI 提示。
#[derive(Debug, Clone, Serialize)]
pub struct NpcapStatus {
    /// 是否检测到 Npcap 安装
    pub installed: bool,
    /// Npcap 版本号（从注册表读取，可能为空即使 installed=true）
    pub version: Option<String>,
    /// wpcap.dll 所在目录（用于诊断；前端不展示）
    pub install_path: Option<String>,
    /// 驱动服务是否注册（installed 但 service 没起来时给出警告）
    pub service_registered: bool,
    /// 当前平台是否支持 Npcap（仅 Windows = true）
    pub supported_platform: bool,
}

impl NpcapStatus {
    #[cfg(not(windows))]
    fn unsupported() -> Self {
        Self {
            installed: false,
            version: None,
            install_path: None,
            service_registered: false,
            supported_platform: false,
        }
    }

    #[cfg(windows)]
    fn not_installed() -> Self {
        Self {
            installed: false,
            version: None,
            install_path: None,
            service_registered: false,
            supported_platform: true,
        }
    }
}

/// 检测 Npcap 是否安装。这是一个轻量的同步检查（仅读文件系统 + 注册表）。
/// 调用方可以在启动时调用一次并缓存。
pub fn detect_npcap() -> NpcapStatus {
    #[cfg(not(windows))]
    {
        return NpcapStatus::unsupported();
    }

    #[cfg(windows)]
    {
        detect_npcap_windows()
    }
}

#[cfg(windows)]
fn detect_npcap_windows() -> NpcapStatus {
    use std::path::Path;

    // 步骤 1：检查 wpcap.dll 是否存在
    let candidate_paths = [
        // Npcap 默认位置（推荐安装方式：未勾选 WinPcap 兼容模式）
        r"C:\Windows\System32\Npcap\wpcap.dll",
        // WinPcap 兼容位置（用户勾选了"Install Npcap in WinPcap API-compatible Mode"）
        r"C:\Windows\System32\wpcap.dll",
    ];

    let install_path = candidate_paths
        .iter()
        .find(|p| Path::new(p).exists())
        .map(|p| {
            Path::new(p)
                .parent()
                .map(|d| d.display().to_string())
                .unwrap_or_else(|| (*p).to_string())
        });

    if install_path.is_none() {
        return NpcapStatus::not_installed();
    }

    // 步骤 2：从注册表读版本号
    let version = read_npcap_version();

    // 步骤 3：检查 npcap 驱动服务是否注册
    let service_registered = is_npcap_service_registered();

    NpcapStatus {
        installed: true,
        version,
        install_path,
        service_registered,
        supported_platform: true,
    }
}

#[cfg(windows)]
fn read_npcap_version() -> Option<String> {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

    // Npcap 安装器写到的位置（依次尝试 64/32 视图）
    let candidate_keys = [
        r"SOFTWARE\WOW6432Node\Npcap",
        r"SOFTWARE\Npcap",
    ];

    for key_path in &candidate_keys {
        if let Ok(key) = hklm.open_subkey(key_path) {
            // Npcap 在该子键下放 (Default) 或 "Version" 值
            // 不同版本字段名略有不同，都试一下
            for value_name in &["", "Version", "DisplayVersion"] {
                if let Ok(version) = key.get_value::<String, _>(*value_name) {
                    let trimmed = version.trim();
                    if !trimmed.is_empty() {
                        return Some(trimmed.to_string());
                    }
                }
            }
        }
    }

    None
}

#[cfg(windows)]
fn is_npcap_service_registered() -> bool {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    // Npcap 驱动服务名固定为 npcap
    hklm.open_subkey(r"SYSTEM\CurrentControlSet\Services\npcap").is_ok()
}
