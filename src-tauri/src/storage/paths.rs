// 数据目录解析（绿色版/便携模式）
//
// 优先级：
// 1. 如果 exe 同目录下存在 `portable.txt` 标记文件 → 使用 `<exe_dir>/data/`
// 2. 否则使用系统 AppData 目录（Windows: %APPDATA%\com.routedoctor.app）
//
// 这种设计兼顾两类用户：
// - 装在 Program Files / 通过安装包安装的：数据走 AppData，符合操作系统规范
// - 解压到 U 盘 / 任意文件夹的：数据跟 exe 走，整个文件夹拷哪里都能用
//
// 业界常见模式：Notepad++、VSCode Portable、JetBrains Toolbox 等都是这种思路。

use std::path::PathBuf;
use tauri::Manager;
use crate::error::{AppError, AppResult};

/// 标记文件名：在 exe 同目录放一个空的 `portable.txt` 即启用便携模式
const PORTABLE_MARKER: &str = "portable.txt";

/// 便携模式下的数据子目录名
const PORTABLE_DATA_SUBDIR: &str = "data";

/// 解析数据目录。返回的路径已确保存在（自动创建）。
///
/// 这是所有持久化数据（数据库、未来的配置文件、日志等）的根目录。
pub fn resolve_data_dir(app_handle: &tauri::AppHandle) -> AppResult<PathBuf> {
    // 尝试获取 exe 所在目录
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()));

    // 如果 exe 同目录有 portable.txt → 走便携模式
    if let Some(dir) = &exe_dir {
        if dir.join(PORTABLE_MARKER).exists() {
            let data_dir = dir.join(PORTABLE_DATA_SUBDIR);
            std::fs::create_dir_all(&data_dir)?;
            log::info!("Using portable data dir: {}", data_dir.display());
            return Ok(data_dir);
        }
    }

    // 否则走系统 AppData
    let app_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Internal(format!("Failed to get app data dir: {}", e)))?;
    std::fs::create_dir_all(&app_dir)?;
    log::info!("Using system data dir: {}", app_dir.display());
    Ok(app_dir)
}

/// 是否处于便携模式（仅供日志/UI 显示用）
pub fn is_portable_mode() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .map(|dir| dir.join(PORTABLE_MARKER).exists())
        .unwrap_or(false)
}
