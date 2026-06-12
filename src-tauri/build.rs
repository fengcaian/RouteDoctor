// Tauri 构建脚本
//
// Windows 上额外做一件事：让链接器能找到 Npcap SDK 的 .lib 文件。
// 优先级（只要任意一项命中即生效）：
// 1. NPCAP_SDK_DIR 环境变量（CI/特殊安装位置使用）
// 2. 项目内的 .npcap-sdk/Lib/x64
// 3. 自动下载 SDK 到 .npcap-sdk/（首次构建时）
//
// 注意：这只是给"开发机/构建机"用的；最终用户机器只需要 Npcap 驱动（运行时），
// 不需要 SDK。

fn main() {
    #[cfg(windows)]
    {
        ensure_npcap_sdk_available();
        println!("cargo:rerun-if-env-changed=NPCAP_SDK_DIR");
    }

    tauri_build::build()
}

#[cfg(windows)]
fn ensure_npcap_sdk_available() {
    use std::path::PathBuf;

    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let local_sdk_root = PathBuf::from(&manifest).join(".npcap-sdk");
    let local_lib_x64 = local_sdk_root.join("Lib").join("x64");

    // 1) 显式环境变量优先
    if let Ok(env_path) = std::env::var("NPCAP_SDK_DIR") {
        let p = PathBuf::from(env_path);
        let candidate = if p.join("Lib").join("x64").join("wpcap.lib").exists() {
            Some(p.join("Lib").join("x64"))
        } else if p.join("wpcap.lib").exists() {
            Some(p.clone())
        } else {
            None
        };
        if let Some(dir) = candidate {
            println!("cargo:rustc-link-search=native={}", dir.display());
            println!("cargo:warning=使用 NPCAP_SDK_DIR 指定的 SDK: {}", dir.display());
            return;
        } else {
            println!(
                "cargo:warning=NPCAP_SDK_DIR={} 不包含 wpcap.lib，将尝试本地 SDK",
                p.display()
            );
        }
    }

    // 2) 项目本地 SDK
    if local_lib_x64.join("wpcap.lib").exists() {
        println!("cargo:rustc-link-search=native={}", local_lib_x64.display());
        println!(
            "cargo:rerun-if-changed={}",
            local_lib_x64.join("wpcap.lib").display()
        );
        return;
    }

    // 3) 都没有 → 尝试自动下载（CI 友好，开发者无感）
    println!("cargo:warning=未找到 Npcap SDK，正在自动下载到 {}", local_sdk_root.display());
    match download_and_extract_npcap_sdk(&local_sdk_root) {
        Ok(()) => {
            if local_lib_x64.join("wpcap.lib").exists() {
                println!("cargo:rustc-link-search=native={}", local_lib_x64.display());
                println!("cargo:warning=Npcap SDK 自动下载完成");
            } else {
                println!("cargo:warning=SDK 下载完成但未找到 wpcap.lib，构建可能失败");
            }
        }
        Err(e) => {
            println!(
                "cargo:warning=Npcap SDK 自动下载失败: {}。请手动运行 src-tauri/scripts/setup-npcap-sdk.ps1",
                e
            );
            // 不直接 panic，让链接阶段给出更详细的错误
        }
    }
}

#[cfg(windows)]
fn download_and_extract_npcap_sdk(target_dir: &std::path::Path) -> Result<(), String> {
    use std::process::Command;

    // 调用 PowerShell 完成下载 + 解压。比起在 build.rs 里引入 reqwest+zip 依赖
    // 更轻量（构建期不引入网络/解压库），而且 Windows 上 PowerShell 一定可用。
    let manifest = std::env::var("CARGO_MANIFEST_DIR").map_err(|e| format!("CARGO_MANIFEST_DIR 缺失: {}", e))?;
    let script = std::path::PathBuf::from(&manifest)
        .join("scripts")
        .join("setup-npcap-sdk.ps1");

    if !script.exists() {
        return Err(format!("setup-npcap-sdk.ps1 不存在于 {}", script.display()));
    }

    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
            &script.to_string_lossy(),
        ])
        .output()
        .map_err(|e| format!("调用 PowerShell 失败: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "脚本退出码 {}: {}",
            output.status.code().unwrap_or(-1),
            stderr.trim()
        ));
    }

    if !target_dir.join("Lib").join("x64").join("wpcap.lib").exists() {
        return Err("脚本执行后仍未找到 wpcap.lib".into());
    }

    Ok(())
}
