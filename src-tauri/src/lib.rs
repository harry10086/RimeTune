pub mod detector;
pub mod deployer;
pub mod backup;
pub mod patcher;
pub mod phrases;
pub mod sync;

use detector::{inspect_environment, EnvironmentStatus};
use deployer::{trigger_deploy, install_linux_plugins as install_linux_plugins_impl, DeployResult};
use backup::{create_snapshot, list_snapshots, restore_snapshot, compare_diff, BackupSnapshot, DiffItem};
use patcher::{load_unified_config, save_unified_config, UnifiedFullConfig};
use phrases::{read_custom_phrases, save_custom_phrases as save_phrases_impl, CustomPhraseItem};
use sync::{
    read_installation_sync_config, save_installation_sync_config,
    trigger_rime_sync as run_rime_sync, RimeSyncConfig,
};

#[tauri::command]
fn get_environment_status() -> EnvironmentStatus {
    inspect_environment()
}

#[tauri::command]
fn get_full_config(user_dir: String) -> UnifiedFullConfig {
    load_unified_config(&user_dir)
}

#[tauri::command]
async fn save_config_and_deploy(user_dir: String, config: UnifiedFullConfig, auto_deploy: bool) -> Result<DeployResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        // 1. 自动写入备份快照
        let _ = create_snapshot(&user_dir, Some("auto_save"));

        // 2. 安全写入 patch 补丁文件
        save_unified_config(&user_dir, config)?;

        // 3. 按需触发静默重新部署
        if auto_deploy {
            let res = trigger_deploy(None);
            Ok(res)
        } else {
            Ok(DeployResult {
                success: true,
                message: "配置已保存至补丁文件（未触发重新部署）".to_string(),
                deployer_used: String::new(),
            })
        }
    }).await.map_err(|e| format!("执行保存与部署任务失败: {}", e))?
}

#[tauri::command]
async fn trigger_weasel_deploy(deployer_path: Option<String>) -> DeployResult {
    tauri::async_runtime::spawn_blocking(move || {
        trigger_deploy(deployer_path)
    }).await.unwrap_or_else(|e| DeployResult {
        success: false,
        message: format!("部署任务调度异常: {}", e),
        deployer_used: String::new(),
    })
}

#[tauri::command]
async fn install_linux_plugins() -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        install_linux_plugins_impl()
    }).await.map_err(|e| format!("执行安装任务失败: {e}"))?
}

#[tauri::command]
fn get_custom_phrases(user_dir: String, file_name: Option<String>, schema_id: Option<String>) -> (Vec<CustomPhraseItem>, Vec<String>, String) {
    read_custom_phrases(&user_dir, file_name.as_deref(), schema_id.as_deref())
}

#[tauri::command]
fn save_phrases(user_dir: String, items: Vec<CustomPhraseItem>, file_name: Option<String>, schema_id: Option<String>) -> Result<String, String> {
    let _ = create_snapshot(&user_dir, Some("before_phrases_save"));
    save_phrases_impl(&user_dir, items, file_name.as_deref(), schema_id.as_deref())
}

#[tauri::command]
fn get_phrase_files_status(user_dir: String, schema_id: Option<String>) -> Vec<phrases::PhraseFileInfo> {
    phrases::list_phrase_files_info(&user_dir, schema_id.as_deref().unwrap_or(""))
}

#[tauri::command]
fn list_backup_snapshots(user_dir: String) -> Vec<BackupSnapshot> {
    list_snapshots(&user_dir)
}

#[tauri::command]
fn restore_backup_snapshot(user_dir: String, snapshot_id: String) -> Result<String, String> {
    restore_snapshot(&user_dir, &snapshot_id)
}

#[tauri::command]
fn get_snapshot_diff(user_dir: String, snapshot_id: String) -> Vec<DiffItem> {
    compare_diff(&user_dir, &snapshot_id)
}

#[tauri::command]
fn create_manual_backup(user_dir: String, note: Option<String>) -> Result<String, String> {
    create_snapshot(&user_dir, note.as_deref())
}

#[tauri::command]
fn open_in_explorer(path: String) -> Result<(), String> {
    let p = std::path::Path::new(&path);
    if !p.exists() {
        return Err(format!("路径不存在: {}", path));
    }

    #[cfg(target_os = "windows")]
    {
        if p.is_file() {
            std::process::Command::new("explorer")
                .arg(format!("/select,{}", path))
                .spawn()
                .map_err(|e| format!("打开资源管理器失败: {}", e))?;
        } else {
            std::process::Command::new("explorer")
                .arg(&path)
                .spawn()
                .map_err(|e| format!("打开资源管理器失败: {}", e))?;
        }
    }

    #[cfg(target_os = "macos")]
    {
        if p.is_file() {
            std::process::Command::new("open")
                .args(["-R", &path])
                .spawn()
                .map_err(|e| format!("打开 Finder 失败: {}", e))?;
        } else {
            std::process::Command::new("open")
                .arg(&path)
                .spawn()
                .map_err(|e| format!("打开 Finder 失败: {}", e))?;
        }
    }

    #[cfg(target_os = "linux")]
    {
        let target = if p.is_file() {
            p.parent().unwrap_or(p)
        } else {
            p
        };
        std::process::Command::new("xdg-open")
            .arg(target)
            .spawn()
            .map_err(|e| format!("打开文件管理器失败: {}", e))?;
    }

    Ok(())
}

#[tauri::command]
fn open_file_in_editor(path: String) -> Result<(), String> {
    let p = std::path::Path::new(&path);
    if !p.exists() {
        return Err(format!("文件不存在: {}", path));
    }

    #[cfg(target_os = "windows")]
    {
        let mut cmd = std::process::Command::new("cmd");
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW: 避免黑色 cmd 窗口闪烁
        cmd.args(["/C", "start", "", &path])
            .spawn()
            .map_err(|e| format!("启动系统编辑器失败: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .args(["-t", &path])
            .spawn()
            .map_err(|e| format!("启动系统文本编辑器失败: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        // 优先尝试 xdg-open
        let xdg_res = std::process::Command::new("xdg-open").arg(&path).spawn();
        if xdg_res.is_err() {
            // 回退尝试 Linux 桌面常见图形文本编辑器 (KDE Kate/KWrite, GNOME Text Editor/GEdit, XFCE Mousepad 等)
            let editors = ["kate", "gedit", "gnome-text-editor", "kwrite", "mousepad", "xed", "leafpad"];
            let mut launched = false;
            for ed in &editors {
                if std::process::Command::new(ed).arg(&path).spawn().is_ok() {
                    launched = true;
                    break;
                }
            }
            if !launched {
                return Err("无法打开系统文本编辑器，未找到 xdg-open 或可用图形编辑器".to_string());
            }
        }
    }

    Ok(())
}

#[tauri::command]
fn open_backups_folder(user_dir: String) -> Result<(), String> {
    let backups_dir = backup::get_backups_dir(&user_dir);
    if !backups_dir.exists() {
        let _ = std::fs::create_dir_all(&backups_dir);
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(backups_dir.to_string_lossy().to_string())
            .spawn()
            .map_err(|e| format!("打开备份目录失败: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&backups_dir)
            .spawn()
            .map_err(|e| format!("打开备份目录失败: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&backups_dir)
            .spawn()
            .map_err(|e| format!("打开备份目录失败: {}", e))?;
    }

    Ok(())
}

#[tauri::command]
fn get_installed_dicts(user_dir: String) -> Vec<patcher::DictFileInfo> {
    patcher::scan_installed_dicts(&user_dir)
}

#[tauri::command]
fn get_system_fonts() -> Vec<String> {
    patcher::enumerate_system_fonts()
}

#[tauri::command]
fn get_schema_features(user_dir: String, schema_id: String) -> Vec<patcher::SchemeFeatureItem> {
    patcher::detect_schema_features(&user_dir, &schema_id)
}

#[tauri::command]
fn get_schema_dict_mounts(user_dir: String, schema_id: String) -> patcher::SchemaDictMountsInfo {
    patcher::detect_schema_dict_mounts(&user_dir, &schema_id)
}

#[tauri::command]
fn toggle_dict_mount_table(user_dir: String, schema_id: String, table_name: String, enabled: bool) -> Result<patcher::SchemaDictMountsInfo, String> {
    patcher::toggle_dict_mount_table(&user_dir, &schema_id, &table_name, enabled)
}

#[tauri::command]
fn get_schema_state_config(user_dir: String, schema_id: String) -> patcher::SchemaStateConfig {
    patcher::get_schema_toggles_and_fuzzy(&user_dir, &schema_id)
}

#[tauri::command]
fn get_sync_config(user_dir: String) -> RimeSyncConfig {
    read_installation_sync_config(&user_dir)
}

#[tauri::command]
fn save_sync_config(user_dir: String, config: RimeSyncConfig) -> Result<(), String> {
    save_installation_sync_config(&user_dir, &config)
}

#[tauri::command]
async fn trigger_rime_sync(deployer_path: Option<String>) -> DeployResult {
    tauri::async_runtime::spawn_blocking(move || {
        run_rime_sync(deployer_path)
    }).await.unwrap_or_else(|e| DeployResult {
        success: false,
        message: format!("同步任务调度异常: {}", e),
        deployer_used: String::new(),
    })
}

#[tauri::command]
fn pick_sync_folder() -> Result<Option<String>, String> {
    sync::pick_sync_folder()
}

#[tauri::command]
fn get_default_preset_schemes(user_dir: String) -> Vec<patcher::ColorSchemeItem> {
    patcher::load_preset_color_schemes(&user_dir)
}

#[tauri::command]
fn import_custom_dict(user_dir: String) -> Result<Option<patcher::CustomDictItem>, String> {
    patcher::import_dict_file_dialog(&user_dir)
}

#[tauri::command]
fn create_empty_custom_dict(user_dir: String, name: String) -> Result<patcher::CustomDictItem, String> {
    patcher::create_empty_dict(&user_dir, &name)
}

#[tauri::command]
fn delete_custom_dict(user_dir: String, file_name: String) -> Result<(), String> {
    patcher::delete_custom_dict_file(&user_dir, &file_name)
}

#[tauri::command]
fn parse_color_scheme_yaml(yaml_str: String) -> Result<patcher::ColorSchemeItem, String> {
    patcher::parse_color_scheme_yaml(&yaml_str)
}

use tauri::Manager;

fn resolve_actual_user_dir(user_dir: Option<&str>) -> Result<String, String> {
    if let Some(d) = user_dir {
        if !d.trim().is_empty() {
            return Ok(d.trim().to_string());
        }
    }
    let (_, detected_user_dir, _) = detector::detect_weasel_paths();
    if let Some(d) = detected_user_dir {
        return Ok(d);
    }
    #[cfg(windows)]
    {
        let appdata = std::env::var("APPDATA").map_err(|_| "无法定位 Rime 用户目录或 APPDATA".to_string())?;
        Ok(std::path::PathBuf::from(appdata).join("Rime").to_string_lossy().to_string())
    }
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").map_err(|_| "无法定位用户 HOME 目录".to_string())?;
        Ok(std::path::PathBuf::from(home).join("Library/Rime").to_string_lossy().to_string())
    }
    #[cfg(target_os = "linux")]
    {
        let home = std::env::var("HOME").map_err(|_| "无法定位用户 HOME 目录".to_string())?;
        let candidates = [
            std::path::PathBuf::from(&home).join(".local/share/fcitx5/rime"),
            std::path::PathBuf::from(&home).join(".config/fcitx5/rime"),
            std::path::PathBuf::from(&home).join(".config/ibus/rime"),
        ];
        let p = candidates.into_iter().find(|p| p.exists()).unwrap_or_else(|| {
            std::path::PathBuf::from(home).join(".local/share/fcitx5/rime")
        });
        Ok(p.to_string_lossy().to_string())
    }
    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    {
        Err("未知系统平台，无法定位 Rime 用户目录".to_string())
    }
}

#[derive(serde::Serialize)]
pub struct TypingStatsDay {
    pub date: String,
    pub chars: u64,
}

#[derive(serde::Serialize)]
pub struct TypingStatsStatus {
    pub enabled: bool,
    pub lua_installed: bool,
    pub data_exists: bool,
    pub total_chars: u64,
    pub file_path: String,
}

/// 读取输入法侧 `input_stats.lua` 写入的每日字数统计（只含日期与字数）。
#[tauri::command]
async fn read_typing_stats(user_dir: Option<String>) -> Result<Vec<TypingStatsDay>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let actual_dir = resolve_actual_user_dir(user_dir.as_deref())?;
        let path = std::path::Path::new(&actual_dir).join("typing_stats.jsonl");
        if !path.exists() {
            return Ok(Vec::new());
        }
        let content = std::fs::read_to_string(&path)
            .map_err(|e| format!("读取 typing_stats.jsonl 失败: {e}"))?;

        let mut daily: std::collections::BTreeMap<String, u64> = std::collections::BTreeMap::new();
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };
            let Some(date) = value.get("date").and_then(|v| v.as_str()) else {
                continue;
            };
            let Some(chars) = value.get("chars").and_then(|v| v.as_u64()) else {
                continue;
            };
            *daily.entry(date.to_string()).or_insert(0) += chars;
        }

        Ok(daily
            .into_iter()
            .map(|(date, chars)| TypingStatsDay { date, chars })
            .collect())
    })
    .await
    .map_err(|e| format!("异步执行读取打字统计失败: {e}"))?
}

/// 查询打字统计模块的启用与数据状态
#[tauri::command]
async fn get_typing_stats_status(user_dir: Option<String>) -> Result<TypingStatsStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let actual_dir = resolve_actual_user_dir(user_dir.as_deref())?;
        let u_path = std::path::Path::new(&actual_dir);
        let lua_file = u_path.join("lua").join("input_stats.lua");
        let lua_installed = lua_file.exists();

        let stats_file = u_path.join("typing_stats.jsonl");
        let data_exists = stats_file.exists();

        let mut total_chars: u64 = 0;
        if data_exists {
            if let Ok(content) = std::fs::read_to_string(&stats_file) {
                for line in content.lines() {
                    if let Ok(value) = serde_json::from_str::<serde_json::Value>(line.trim()) {
                        if let Some(chars) = value.get("chars").and_then(|v| v.as_u64()) {
                            total_chars += chars;
                        }
                    }
                }
            }
        }

        // 轻量直接检测打字统计是否在补丁中挂载，避免阻塞主线程运行庞大的 load_unified_config
        let mut stats_patch_detected = false;
        let check_files = [
            "rime_ice.custom.yaml",
            "rime_frost.custom.yaml",
            "rime_mint.custom.yaml",
            "wanxiang.custom.yaml",
            "default.custom.yaml",
            "luna_pinyin.custom.yaml",
        ];
        for fname in &check_files {
            if let Ok(c) = std::fs::read_to_string(u_path.join(fname)) {
                if c.contains("lua_filter@*input_stats") {
                    stats_patch_detected = true;
                    break;
                }
            }
        }
        if !stats_patch_detected {
            if let Ok(entries) = std::fs::read_dir(u_path) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if let Some(name) = p.file_name().and_then(|s| s.to_str()) {
                        if name.ends_with(".custom.yaml") {
                            if let Ok(c) = std::fs::read_to_string(&p) {
                                if c.contains("lua_filter@*input_stats") {
                                    stats_patch_detected = true;
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }

        let enabled = lua_installed && stats_patch_detected;

        Ok(TypingStatsStatus {
            enabled,
            lua_installed,
            data_exists,
            total_chars,
            file_path: stats_file.to_string_lossy().to_string(),
        })
    })
    .await
    .map_err(|e| format!("异步执行查询打字统计状态失败: {e}"))?
}

/// 一键开启或停用打字统计，自动维护 Lua 采集器与补丁，并静默重新部署
#[tauri::command]
async fn set_typing_stats_enabled(user_dir: Option<String>, enabled: bool) -> Result<DeployResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let actual_dir = resolve_actual_user_dir(user_dir.as_deref())?;

        // 1. 读取当前统一配置并修改开关
        let mut config = patcher::load_unified_config(&actual_dir);
        config.rime_ice_toggles.enable_typing_stats = enabled;

        // 2. 自动写入备份快照
        let _ = backup::create_snapshot(
            &actual_dir,
            Some(if enabled { "enable_typing_stats" } else { "disable_typing_stats" }),
        );

        // 3. 保存补丁并同步 Lua 文件
        patcher::save_unified_config(&actual_dir, config)?;

        // 4. 自动触发重新部署使之立即生效
        let res = deployer::trigger_deploy(None);
        Ok(res)
    })
    .await
    .map_err(|e| format!("调度打字统计切换任务失败: {e}"))?
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct UpdateCheckResult {
    pub current_version: String,
    pub latest_version: String,
    pub has_update: bool,
    pub release_notes: String,
    pub release_url: String,
    pub published_at: String,
}

#[tauri::command]
async fn check_for_updates() -> Result<UpdateCheckResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let current_version = env!("CARGO_PKG_VERSION").to_string();

        // 访问 GitHub Releases API 获取最新版本
        let url = "https://api.github.com/repos/harry10086/RimeTune/releases/latest";
        
        // 使用原生 HTTP 请求（在 Windows/Linux/macOS 下通用）
        #[cfg(windows)]
        let output = {
            use std::os::windows::process::CommandExt;
            let mut cmd = std::process::Command::new("powershell");
            cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW: 静默执行，彻底杜绝黑色控制台窗口闪烁
            cmd.args([
                "-NoProfile",
                "-WindowStyle",
                "Hidden",
                "-Command",
                &format!(
                    r#"[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12; $wc = New-Object Net.WebClient; $wc.Headers.Add('User-Agent', 'RimeTune-App'); try {{ $wc.DownloadString('{}') }} catch {{ '' }}"#,
                    url
                ),
            ])
            .output()
            .map_err(|e| format!("检查更新请求失败: {}", e))?
        };

        #[cfg(not(windows))]
        let output = std::process::Command::new("curl")
            .args(["-s", "-H", "User-Agent: RimeTune-App", url])
            .output()
            .map_err(|e| format!("检查更新请求失败: {}", e))?;

        let body = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if body.is_empty() || !body.starts_with('{') {
            return Err("无法连接至 GitHub Releases 服务，请检查网络或稍后重试".to_string());
        }

        let json_val: serde_json::Value = serde_json::from_str(&body)
            .map_err(|e| format!("解析版本信息失败: {}", e))?;

        let tag_name = json_val.get("tag_name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();

        if tag_name.is_empty() {
            return Err("未获取到有效的发布版本号".to_string());
        }

        let latest_version = tag_name.trim_start_matches('v').to_string();
        let release_url = json_val.get("html_url")
            .and_then(|v| v.as_str())
            .unwrap_or("https://github.com/harry10086/RimeTune/releases")
            .to_string();

        let release_notes = json_val.get("body")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let published_at = json_val.get("published_at")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // 简要版本号对比 (major.minor.patch)
        let parse_ver = |v: &str| -> Vec<u32> {
            v.split('.')
                .filter_map(|s| s.split(|c: char| !c.is_ascii_digit()).next().and_then(|n| n.parse::<u32>().ok()))
                .collect()
        };

        let curr_parts = parse_ver(&current_version);
        let latest_parts = parse_ver(&latest_version);

        let mut has_update = false;
        for i in 0..latest_parts.len().max(curr_parts.len()) {
            let l = latest_parts.get(i).copied().unwrap_or(0);
            let c = curr_parts.get(i).copied().unwrap_or(0);
            if l > c {
                has_update = true;
                break;
            } else if l < c {
                has_update = false;
                break;
            }
        }

        Ok(UpdateCheckResult {
            current_version,
            latest_version,
            has_update,
            release_notes,
            release_url,
            published_at,
        })
    })
    .await
    .map_err(|e| format!("检查更新任务调度失败: {}", e))?
}

#[cfg(target_os = "linux")]
fn ensure_linux_desktop_icon() {
    let home = std::env::var("HOME").unwrap_or_default();
    if home.is_empty() {
        return;
    }
    let home_path = std::path::Path::new(&home);
    let icon_dir = home_path.join(".local/share/icons/hicolor/128x128/apps");
    let _ = std::fs::create_dir_all(&icon_dir);

    let icon_bytes = include_bytes!("../icons/128x128.png");
    let icon_path1 = icon_dir.join("com.rimetune.desktop.png");
    let icon_path2 = icon_dir.join("rimetune.png");
    let _ = std::fs::write(&icon_path1, icon_bytes);
    let _ = std::fs::write(&icon_path2, icon_bytes);

    let apps_dir = home_path.join(".local/share/applications");
    let _ = std::fs::create_dir_all(&apps_dir);

    // 获取当前正在运行的实际二进制绝对路径
    let exe_path = std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| format!("{}/桌面/RimeTune", home));

    let desktop_content = format!(
        "[Desktop Entry]\n\
         Name=RimeTune\n\
         GenericName=Rime Configuration Tool\n\
         Comment=中州韵配置中心\n\
         Exec=\"{}\" %U\n\
         Icon={}\n\
         Terminal=false\n\
         Type=Application\n\
         Categories=Utility;Settings;\n\
         StartupWMClass=rimetune\n\
         StartupNotify=true\n",
        exe_path,
        icon_path1.to_string_lossy()
    );

    let desktop_file1 = apps_dir.join("com.rimetune.desktop.desktop");
    let desktop_file2 = apps_dir.join("rimetune.desktop");
    let _ = std::fs::write(&desktop_file1, &desktop_content);
    let _ = std::fs::write(&desktop_file2, &desktop_content);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(target_os = "linux")]
    ensure_linux_desktop_icon();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                if let Some(icon) = app.default_window_icon() {
                    let _ = window.set_icon(icon.clone());
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_environment_status,
            get_full_config,
            save_config_and_deploy,
            trigger_weasel_deploy,
            get_custom_phrases,
            save_phrases,
            list_backup_snapshots,
            restore_backup_snapshot,
            get_snapshot_diff,
            create_manual_backup,
            open_in_explorer,
            open_file_in_editor,
            open_backups_folder,
            get_installed_dicts,
            get_system_fonts,
            get_schema_features,
            get_schema_dict_mounts,
            toggle_dict_mount_table,
            get_schema_state_config,
            get_sync_config,
            save_sync_config,
            trigger_rime_sync,
            pick_sync_folder,
            get_default_preset_schemes,
            import_custom_dict,
            create_empty_custom_dict,
            delete_custom_dict,
            parse_color_scheme_yaml,
            get_phrase_files_status,
            read_typing_stats,
            get_typing_stats_status,
            set_typing_stats_enabled,
            check_for_updates,
            install_linux_plugins,
        ])
        .run(tauri::generate_context!())
        .expect("error while running WeaselTune");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_environment_detection() {
        let env = inspect_environment();
        println!("Detected env: {:?}", env);
        assert!(!env.platform.is_empty());
        assert!(!env.active_schema_id.is_empty());
        assert!(!env.active_schema_name.is_empty());
    }

    #[test]
    fn test_config_load() {
        let env = inspect_environment();
        let config = load_unified_config(&env.rime_user_dir);
        assert!(!config.preset_schemes.is_empty());
        assert!(config.style.page_size >= 3);
        assert!(!config.schemas.is_empty());
    }

    #[test]
    fn test_custom_phrase_read() {
        let env = inspect_environment();
        let (phrases, _, active_file) = read_custom_phrases(&env.rime_user_dir, None, Some(&env.active_schema_id));
        println!("Custom phrases count: {}, active file: {}", phrases.len(), active_file);
        let statuses = phrases::list_phrase_files_info(&env.rime_user_dir, &env.active_schema_id);
        assert_eq!(statuses.len(), 2);
    }

    #[test]
    fn test_backup_and_diff() {
        let temp_dir = std::env::temp_dir().join("weaseltune_test_env");
        let _ = std::fs::create_dir_all(&temp_dir);
        let sample_file = temp_dir.join("weasel.custom.yaml");
        let _ = std::fs::write(&sample_file, "patch:\n  style/page_size: 5\n");

        let snap_id = create_snapshot(temp_dir.to_str().unwrap(), Some("unit_test")).unwrap();
        let snaps = list_snapshots(temp_dir.to_str().unwrap());
        assert!(!snaps.is_empty());

        let diffs = compare_diff(temp_dir.to_str().unwrap(), &snap_id);
        assert!(diffs.is_empty()); // 此时无差异

        // 改变文件后比较
        let _ = std::fs::write(&sample_file, "patch:\n  style/page_size: 7\n");
        let diffs_after = compare_diff(temp_dir.to_str().unwrap(), &snap_id);
        assert_eq!(diffs_after.len(), 1);

        // 回滚
        let restore_res = restore_snapshot(temp_dir.to_str().unwrap(), &snap_id);
        assert!(restore_res.is_ok());

        let restored_content = std::fs::read_to_string(&sample_file).unwrap();
        assert!(restored_content.contains("page_size: 5"));

        // 清理
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_color_hex_formatting() {
        use patcher::to_weasel_hex_formatted;
        // ABGR / BGR: #RRGGBB -> 0xBBGGRR
        // Red #FF0000 -> 0x0000ff
        assert_eq!(to_weasel_hex_formatted("#FF0000", Some("abgr")), "0x0000ff");
        // Blue #0000FF -> 0xff0000
        assert_eq!(to_weasel_hex_formatted("#0000FF", Some("abgr")), "0xff0000");
        // Cool breeze back color #FBFBFF -> 0xfffbfb
        assert_eq!(to_weasel_hex_formatted("#FBFBFF", Some("abgr")), "0xfffbfb");
        // RGBA: #FF0000 -> 0xff0000
        assert_eq!(to_weasel_hex_formatted("#FF0000", Some("rgba")), "0xff0000");
    }

    #[test]
    fn test_is_scheme_modified() {
        use patcher::{is_scheme_modified, get_builtin_schemes};
        let defaults = get_builtin_schemes();
        let mut cool_breeze = defaults.iter().find(|s| s.id == "cool_breeze").unwrap().clone();
        
        // 初始未修改
        assert!(!is_scheme_modified(&cool_breeze, &defaults));

        // 修改文字颜色
        cool_breeze.text_color = "#00FF00".to_string();
        assert!(is_scheme_modified(&cool_breeze, &defaults));
    }

    #[test]
    fn test_sync_config_read_write() {
        let temp_dir = std::env::temp_dir().join("weaseltune_test_sync");
        let _ = std::fs::create_dir_all(&temp_dir);
        let u_str = temp_dir.to_str().unwrap();

        let initial_cfg = read_installation_sync_config(u_str);
        assert_eq!(initial_cfg.installation_id, "");
        assert_eq!(initial_cfg.sync_dir, "");

        let new_cfg = RimeSyncConfig {
            installation_id: "PC-Work".to_string(),
            sync_dir: "D:\\RimeSync".to_string(),
        };
        assert!(save_installation_sync_config(u_str, &new_cfg).is_ok());

        let loaded = read_installation_sync_config(u_str);
        assert_eq!(loaded.installation_id, "PC-Work");
        assert_eq!(loaded.sync_dir, "D:\\RimeSync");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_custom_dict_lifecycle() {
        let temp_dir = std::env::temp_dir().join("weaseltune_test_custom_dict");
        let _ = std::fs::create_dir_all(&temp_dir);
        let u_str = temp_dir.to_str().unwrap();

        // 1. 新建词库
        let res = patcher::create_empty_dict(u_str, "medical_terms");
        assert!(res.is_ok());
        let item = res.unwrap();
        assert_eq!(item.file_name, "medical_terms.dict.yaml");
        assert_eq!(item.name, "custom_dicts/medical_terms");
        assert!(std::path::Path::new(&item.full_path).exists());

        // 2. 扫描词库
        let list = patcher::scan_custom_dicts(u_str);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].file_name, "medical_terms.dict.yaml");

        // 3. 删除词库
        let del_res = patcher::delete_custom_dict_file(u_str, "medical_terms.dict.yaml");
        assert!(del_res.is_ok());
        assert!(!std::path::Path::new(&item.full_path).exists());

        let list_after = patcher::scan_custom_dicts(u_str);
        assert!(list_after.is_empty());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_parse_color_scheme_yaml() {
        let yaml_sample = r#"
    name: "抹茶/matcha"
    author: "AIME"
    color_format: argb
    back_color: 0xF5F4F9F1
    border_color: 0x1F4D7C0F
    preedit_back_color: 0x00000000
    text_color: 0xFF4D7C0F
    hilited_text_color: 0xFF365314
    hilited_back_color: 0x1F65A30D
    candidate_text_color: 0xFF1A2E05
    comment_text_color: 0xFF84A36B
    label_color: 0xFF9CB88A
    hilited_candidate_back_color: 0xFF4D7C0F
    hilited_candidate_text_color: 0xFFF7FEE7
    hilited_comment_text_color: 0xFFD9F99D
    hilited_candidate_label_color: 0xFFD9F99D
"#;
        let res = patcher::parse_color_scheme_yaml(yaml_sample);
        assert!(res.is_ok(), "Failed to parse: {:?}", res.err());
        let scheme = res.unwrap();
        assert_eq!(scheme.id, "matcha");
        assert_eq!(scheme.name, "抹茶/matcha");
        assert_eq!(scheme.author, "AIME");
        assert_eq!(scheme.color_format.as_deref(), Some("argb"));
        assert_eq!(scheme.back_color, "#F4F9F1");
        assert_eq!(scheme.text_color, "#4D7C0F");
        assert_eq!(scheme.candidate_text_color, "#1A2E05");
        assert_eq!(scheme.hilited_text_color, "#365314");
        assert_eq!(scheme.hilited_candidate_text_color, Some("#F7FEE7".to_string()));
        assert_eq!(scheme.hilited_candidate_back_color, Some("#4D7C0F".to_string()));
        assert_eq!(scheme.hilited_comment_text_color, Some("#D9F99D".to_string()));

        // Test missing color fields defaulting to black and white
        let minimal_yaml = r#"
name: "极简白"
author: "Tester"
"#;
        let min_res = patcher::parse_color_scheme_yaml(minimal_yaml);
        assert!(min_res.is_ok());
        let min_scheme = min_res.unwrap();
        assert_eq!(min_scheme.back_color, "#FFFFFF");
        assert_eq!(min_scheme.text_color, "#000000");

        // Test invalid yaml syntax
        let invalid = "name: [unclosed list";
        assert!(patcher::parse_color_scheme_yaml(invalid).is_err());
    }

    #[test]
    fn test_yaml_escape_and_quote_issue_1() {
        use patcher::{yaml_escape_str, validate_yaml};

        // 薄荷拼音中的复杂作者字段（含双引号，复现 Issue #1）
        let raw_author = r#"Mintimate <"Mintimate's Blog">"#;
        let escaped = yaml_escape_str(raw_author);
        assert_eq!(escaped, r#"Mintimate <\"Mintimate's Blog\">"#);

        // 拼接为 YAML 补丁
        let yaml_snippet = format!(
            "patch:\n  \"preset_color_schemes/mint_light_green/author\": \"{}\"\n",
            escaped
        );

        // 校验 YAML 合法性
        let val_res = validate_yaml(&yaml_snippet, "weasel.custom.yaml");
        assert!(val_res.is_ok(), "YAML 验证失败: {:?}", val_res.err());

        // 解析并验证反序列化出的内容恢复为原始字符串
        let parsed: serde_yaml::Value = serde_yaml::from_str(&yaml_snippet).unwrap();
        let author_val = parsed
            .get("patch")
            .and_then(|p| p.get("preset_color_schemes/mint_light_green/author"))
            .and_then(|v| v.as_str())
            .unwrap();
        assert_eq!(author_val, raw_author);
    }

    #[test]
    fn test_typing_stats_toggle() {
        let temp_dir = std::env::temp_dir().join("weaseltune_test_stats_toggle");
        let _ = std::fs::remove_dir_all(&temp_dir);
        let _ = std::fs::create_dir_all(&temp_dir);
        let u_str = temp_dir.to_str().unwrap();

        // 模拟已存在方案文件
        std::fs::write(temp_dir.join("rime_ice.schema.yaml"), "schema:\n  schema_id: rime_ice\n").unwrap();

        // 1. 开启打字统计
        let mut config = patcher::load_unified_config(u_str);
        config.rime_ice_toggles.enable_typing_stats = true;
        let save_res = patcher::save_unified_config(u_str, config);
        assert!(save_res.is_ok());

        // 检查 lua 文件是否存在
        let lua_file = temp_dir.join("lua").join("input_stats.lua");
        assert!(lua_file.exists());

        // 检查 rime_ice.custom.yaml 是否挂载了 filter
        let custom_content = std::fs::read_to_string(temp_dir.join("rime_ice.custom.yaml")).unwrap();
        assert!(custom_content.contains("lua_filter@*input_stats"));

        // 2. 关闭打字统计
        let mut config_disabled = patcher::load_unified_config(u_str);
        assert!(config_disabled.rime_ice_toggles.enable_typing_stats);
        config_disabled.rime_ice_toggles.enable_typing_stats = false;
        let save_dis_res = patcher::save_unified_config(u_str, config_disabled);
        assert!(save_dis_res.is_ok());

        let custom_disabled_content = std::fs::read_to_string(temp_dir.join("rime_ice.custom.yaml")).unwrap();
        assert!(!custom_disabled_content.contains("lua_filter@*input_stats"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_issue_3_wanxiang_lite_and_frost_save_verified() {
        let temp_dir = std::env::temp_dir().join("weaseltune_test_issue_3");
        let _ = std::fs::remove_dir_all(&temp_dir);
        let _ = std::fs::create_dir_all(&temp_dir);
        let u_str = temp_dir.to_str().unwrap();

        // 1. 模拟 wanxiang_lite.schema.yaml 和 default.custom.yaml
        let wanxiang_content = r#"schema:
  schema_id: wanxiang_lite
  name: 万象拼音·无词
switches:
  - name: ascii_mode
    reset: 0
  - name: ascii_punct
    reset: 0
  - name: full_shape
    reset: 0
  - options: [ zh_simp, zh_tw, zh_hk ]
    reset: 0
  - name: emoji
    reset: 0
  - name: super_tips
    reset: 0
"#;
        std::fs::write(temp_dir.join("wanxiang_lite.schema.yaml"), wanxiang_content).unwrap();
        std::fs::write(temp_dir.join("default.custom.yaml"), "patch:\n  schema_list:\n    - schema: wanxiang_lite\n").unwrap();

        // 2. 加载配置并修改万象拼音选项
        let mut config = patcher::load_unified_config(u_str);
        config.rime_ice_toggles.emoji = true;
        config.rime_ice_toggles.super_tips = true;
        config.rime_ice_toggles.traditionalization = true;
        config.rime_ice_toggles.enable_grammar = true;
        config.fuzzy_pinyin.ian_iang = true;

        // 3. 保存配置
        let save_res = patcher::save_unified_config(u_str, config);
        assert!(save_res.is_ok(), "save_unified_config failed: {:?}", save_res);

        // 4. 验证生成的 custom.yaml 不含重复 key，并能再次被 load_unified_config 成功加载
        let custom_file = temp_dir.join("wanxiang_lite.custom.yaml");
        assert!(custom_file.exists());
        let custom_content = std::fs::read_to_string(&custom_file).unwrap();
        println!("wanxiang_lite.custom.yaml content:\n{}", custom_content);

        let reloaded = patcher::load_unified_config(u_str);
        assert!(reloaded.rime_ice_toggles.emoji, "emoji should be true");
        assert!(reloaded.rime_ice_toggles.super_tips, "super_tips should be true");
        assert!(reloaded.rime_ice_toggles.traditionalization, "traditionalization should be true");
        assert!(reloaded.fuzzy_pinyin.ian_iang, "ian_iang should be true");
        assert!(reloaded.rime_ice_toggles.enable_grammar, "enable_grammar should be true");

        // 5. 验证 rime_frost 的语法模型（Octagram）保存与加载
        let frost_dir = std::env::temp_dir().join("weaseltune_test_issue_3_frost");
        let _ = std::fs::remove_dir_all(&frost_dir);
        let _ = std::fs::create_dir_all(&frost_dir);
        let f_str = frost_dir.to_str().unwrap();

        std::fs::write(frost_dir.join("rime_frost.schema.yaml"), "schema:\n  schema_id: rime_frost\n").unwrap();
        std::fs::write(frost_dir.join("default.custom.yaml"), "patch:\n  schema_list:\n    - schema: rime_frost\n").unwrap();

        let mut frost_cfg = patcher::load_unified_config(f_str);
        frost_cfg.rime_ice_toggles.enable_grammar = true;
        let frost_save = patcher::save_unified_config(f_str, frost_cfg);
        assert!(frost_save.is_ok());

        let frost_custom = std::fs::read_to_string(frost_dir.join("rime_frost.custom.yaml")).unwrap();
        assert!(frost_custom.contains(r#""grammar/language": "zh-hans-t-essay-bgw""#));

        let frost_reloaded = patcher::load_unified_config(f_str);
        assert!(frost_reloaded.rime_ice_toggles.enable_grammar, "rime_frost enable_grammar should reload as true");

        let _ = std::fs::remove_dir_all(&temp_dir);
        let _ = std::fs::remove_dir_all(&frost_dir);
    }

    #[test]
    fn test_schema_switches_spelling_hints_and_key_bindings() {
        let temp_dir = std::env::temp_dir().join("rimetune_test_switches_keys");
        let _ = std::fs::remove_dir_all(&temp_dir);
        let _ = std::fs::create_dir_all(&temp_dir);
        let u_str = temp_dir.to_str().unwrap();

        let luna_content = r#"
schema:
  schema_id: luna_pinyin_simp
  name: 朙月拼音·简化字
switches:
  - name: ascii_mode
    reset: 0
    states: ["中文", "西文"]
  - name: full_shape
    states: ["半角", "全角"]
  - name: zh_simp
    reset: 1
    states: ["漢字", "汉字"]
  - name: ascii_punct
    states: ["，。", "，．"]
"#;
        std::fs::write(temp_dir.join("luna_pinyin_simp.schema.yaml"), luna_content).unwrap();

        let mut config = patcher::load_unified_config(u_str);
        config.schemas = vec![
            patcher::SchemaItem {
                id: "luna_pinyin_simp".to_string(),
                name: "朙月拼音·简化字".to_string(),
                enabled: true,
                description: "通用全拼方案".to_string(),
                file_path: temp_dir.join("luna_pinyin_simp.schema.yaml").to_string_lossy().to_string(),
            }
        ];
        config.rime_ice_toggles.spelling_hints = true;
        config.rime_ice_toggles.traditionalization = false; // 用户要简体，zh_simp 应为 1
        config.rime_ice_toggles.full_shape = true;
        config.rime_ice_toggles.ascii_punct = true;
        config.key_bindings.page_up_down_keys = vec!["comma_period".to_string(), "minus_equal".to_string()];

        let res = patcher::save_unified_config(u_str, config);
        assert!(res.is_ok(), "save_unified_config failed: {:?}", res);

        // 1. 验证 default.custom.yaml 含有 page_up_down_keys 且不含 switches/@name
        let def_custom = std::fs::read_to_string(temp_dir.join("default.custom.yaml")).unwrap();
        assert!(!def_custom.contains("@name"), "default.custom.yaml must not contain invalid @name switches");
        assert!(def_custom.contains("Page_Up"), "default.custom.yaml must contain global Page_Up bindings");
        assert!(def_custom.contains("comma"), "default.custom.yaml must contain comma paging key");

        // 2. 验证 luna_pinyin_simp.custom.yaml
        let luna_custom = std::fs::read_to_string(temp_dir.join("luna_pinyin_simp.custom.yaml")).unwrap();
        assert!(luna_custom.contains(r#""translator/spelling_hints": 8"#), "luna_custom must contain spelling_hints: 8");
        assert!(luna_custom.contains(r#""switches/@1/reset": 1"#), "full_shape reset should be 1");
        assert!(luna_custom.contains(r#""switches/@2/reset": 1"#), "zh_simp reset should be 1 for simplified Chinese");
        assert!(luna_custom.contains(r#""switches/@3/reset": 1"#), "ascii_punct reset should be 1");

        // 3. 验证 user.yaml 状态写入与读取
        let user_yaml = std::fs::read_to_string(temp_dir.join("user.yaml")).unwrap();
        assert!(user_yaml.contains("full_shape: true"));
        assert!(user_yaml.contains("ascii_punct: true"));
        assert!(user_yaml.contains("zh_simp: true"));
        assert!(user_yaml.contains("traditionalization: false"));
        assert!(user_yaml.contains("previously_selected_schema: luna_pinyin_simp"));

        let reloaded = patcher::load_unified_config(u_str);
        assert!(reloaded.rime_ice_toggles.spelling_hints);
        assert!(reloaded.rime_ice_toggles.full_shape);
        assert!(reloaded.rime_ice_toggles.ascii_punct);
        assert!(!reloaded.rime_ice_toggles.traditionalization);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_primary_schema_user_yaml_sync() {
        use crate::patcher::SchemaItem;
        let temp_dir = std::env::temp_dir().join(format!("rimetune_schema_sync_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let u_str = temp_dir.to_str().unwrap();

        std::fs::write(temp_dir.join("rime_ice.schema.yaml"), "schema:\n  schema_id: rime_ice\n  name: 雾凇拼音\n").unwrap();
        std::fs::write(temp_dir.join("luna_pinyin_simp.schema.yaml"), "schema:\n  schema_id: luna_pinyin_simp\n  name: 朙月拼音·简化字\n").unwrap();

        let mut config = patcher::load_unified_config(u_str);
        config.schemas = vec![
            SchemaItem {
                id: "rime_ice".to_string(),
                name: "雾凇拼音".to_string(),
                enabled: true,
                description: "".to_string(),
                file_path: temp_dir.join("rime_ice.schema.yaml").to_str().unwrap().to_string(),
            },
            SchemaItem {
                id: "luna_pinyin_simp".to_string(),
                name: "朙月拼音·简化字".to_string(),
                enabled: true,
                description: "".to_string(),
                file_path: temp_dir.join("luna_pinyin_simp.schema.yaml").to_str().unwrap().to_string(),
            },
        ];

        let res = patcher::save_unified_config(u_str, config.clone());
        assert!(res.is_ok());

        // 验证 user.yaml 正确写入了 rime_ice
        let user_yaml = std::fs::read_to_string(temp_dir.join("user.yaml")).unwrap();
        assert!(user_yaml.contains("previously_selected_schema: rime_ice"), "user.yaml must record rime_ice as previously_selected_schema");
        assert!(user_yaml.contains("schema_access_time"), "user.yaml must have schema_access_time");

        let scanned = patcher::scan_actual_installed_schemas(u_str);
        assert_eq!(scanned[0].id, "rime_ice");

        // 切换顺序：将 luna_pinyin_simp 设为第 1 方案
        config.schemas = vec![
            SchemaItem {
                id: "luna_pinyin_simp".to_string(),
                name: "朙月拼音·简化字".to_string(),
                enabled: true,
                description: "".to_string(),
                file_path: temp_dir.join("luna_pinyin_simp.schema.yaml").to_str().unwrap().to_string(),
            },
            SchemaItem {
                id: "rime_ice".to_string(),
                name: "雾凇拼音".to_string(),
                enabled: true,
                description: "".to_string(),
                file_path: temp_dir.join("rime_ice.schema.yaml").to_str().unwrap().to_string(),
            },
        ];
        let res2 = patcher::save_unified_config(u_str, config);
        assert!(res2.is_ok());

        let user_yaml2 = std::fs::read_to_string(temp_dir.join("user.yaml")).unwrap();
        assert!(user_yaml2.contains("previously_selected_schema: luna_pinyin_simp"), "user.yaml must switch previously_selected_schema to luna_pinyin_simp");

        let scanned2 = patcher::scan_actual_installed_schemas(u_str);
        assert_eq!(scanned2[0].id, "luna_pinyin_simp");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_double_pinyin_ice_family_and_switcher_hotkeys() {
        let temp_dir = std::env::temp_dir().join(format!("rimetune_test_dp_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let u_str = temp_dir.to_str().unwrap();

        // 模拟用户目录下安装了 rime_ice 和 double_pinyin_mspy
        std::fs::write(temp_dir.join("rime_ice.schema.yaml"), "schema:\n  schema_id: rime_ice\n  name: 雾凇拼音\nswitches:\n  - name: ascii_mode\n  - name: ascii_punct\n  - name: traditionalization\n  - name: emoji\n  - name: full_shape\n  - name: search_single_char\n  - name: dict_comment\n").unwrap();
        std::fs::write(temp_dir.join("double_pinyin_mspy.schema.yaml"), "schema:\n  schema_id: double_pinyin_mspy\n  name: 微软双拼\nswitches:\n  - name: ascii_mode\n  - name: ascii_punct\n  - name: traditionalization\n  - name: emoji\n  - name: full_shape\n  - name: search_single_char\n  - name: dict_comment\n").unwrap();

        let mut config = patcher::load_unified_config(u_str);
        config.schemas = vec![
            patcher::SchemaItem {
                id: "double_pinyin_mspy".to_string(),
                name: "微软双拼".to_string(),
                enabled: true,
                description: "".to_string(),
                file_path: temp_dir.join("double_pinyin_mspy.schema.yaml").to_str().unwrap().to_string(),
            },
            patcher::SchemaItem {
                id: "rime_ice".to_string(),
                name: "雾凇拼音".to_string(),
                enabled: true,
                description: "".to_string(),
                file_path: temp_dir.join("rime_ice.schema.yaml").to_str().unwrap().to_string(),
            },
        ];

        // 开启万象语言模型与词典释义，配置 switcher_hotkeys 包含 Control+Shift+grave
        config.rime_ice_toggles.enable_grammar = true;
        config.rime_ice_toggles.grammar_model = "wanxiang-lts-zh-hans".to_string();
        config.rime_ice_toggles.dict_comment_chinese_to_english = true;
        config.key_bindings.switcher_hotkeys = vec!["Control+Shift+grave".to_string(), "F4".to_string()];
        config.key_bindings.shift_l = "commit_text".to_string();
        config.key_bindings.shift_r = "inline_ascii".to_string();

        let res = patcher::save_unified_config(u_str, config.clone());
        assert!(res.is_ok(), "save_unified_config failed: {:?}", res);

        // 1. 验证 double_pinyin_mspy.custom.yaml 成功生成且完整包含万象语言模型与词典释义
        let dp_custom = std::fs::read_to_string(temp_dir.join("double_pinyin_mspy.custom.yaml")).expect("double_pinyin_mspy.custom.yaml should exist");
        assert!(dp_custom.contains("grammar/language"), "double_pinyin_mspy must contain grammar/language");
        assert!(dp_custom.contains("wanxiang-lts-zh-hans"), "double_pinyin_mspy must configure wanxiang model");
        assert!(dp_custom.contains("dict_comment_filter"), "double_pinyin_mspy must contain dict_comment_filter");

        // 2. 验证 default.custom.yaml 中 Control+Shift+grave 自动补全了 Linux/X11 的 asciitilde 键位别名
        let default_custom = std::fs::read_to_string(temp_dir.join("default.custom.yaml")).unwrap();
        assert!(default_custom.contains("Control+asciitilde"), "default.custom.yaml must contain Control+asciitilde for Linux keysym compatibility");
        assert!(default_custom.contains("Control+Shift+asciitilde"), "default.custom.yaml must contain Control+Shift+asciitilde");
        assert!(default_custom.contains("Shift_L: commit_text"), "default.custom.yaml must contain Shift_L");
        assert!(default_custom.contains("Shift_R: inline_ascii"), "default.custom.yaml must contain Shift_R");

        // 3. 验证再次 load_unified_config 时能无缝反读，且内部 asciitilde 别名不污染 UI 状态
        let reloaded = patcher::load_unified_config(u_str);
        assert!(reloaded.rime_ice_toggles.enable_grammar);
        assert_eq!(reloaded.rime_ice_toggles.grammar_model, "wanxiang-lts-zh-hans");
        assert!(reloaded.key_bindings.switcher_hotkeys.contains(&"Control+Shift+grave".to_string()));
        assert!(!reloaded.key_bindings.switcher_hotkeys.contains(&"Control+asciitilde".to_string()));
        assert_eq!(reloaded.key_bindings.shift_l, "commit_text");
        assert_eq!(reloaded.key_bindings.shift_r, "inline_ascii");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_dict_mount_toggle_and_custom_dict_persistence() {
        let temp_dir = std::env::temp_dir().join(format!("rimetune_dict_sync_test_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let u_str = temp_dir.to_str().unwrap();

        // 准备初始 rime_ice.dict.yaml
        let initial_dict = r#"# Rime dictionary
---
name: rime_ice
version: "0.1"
import_tables:
  - cn_dicts/8105
  # - cn_dicts/41448
  - cn_dicts/base
  - cn_dicts/ext
  - cn_dicts/tencent
  - cn_dicts/others
...
"#;
        std::fs::write(temp_dir.join("rime_ice.dict.yaml"), initial_dict).unwrap();

        // 1. 验证初始状态探测
        let mut cfg = patcher::load_unified_config(u_str);
        assert!(!cfg.rime_ice_toggles.dict_large_char, "41448 should initially be disabled");
        assert!(cfg.rime_ice_toggles.dict_base, "base should initially be enabled");
        assert!(cfg.rime_ice_toggles.dict_ext, "ext should initially be enabled");
        assert!(cfg.rime_ice_toggles.dict_tencent, "tencent should initially be enabled");
        assert!(cfg.rime_ice_toggles.dict_others, "others should initially be enabled");

        // 2. 关闭 cn_dicts/ext
        let mounts = patcher::toggle_dict_mount_table(u_str, "rime_ice", "cn_dicts/ext", false).unwrap();
        let ext_item = mounts.mounted_tables.iter().find(|t| t.name == "cn_dicts/ext").unwrap();
        assert!(!ext_item.enabled, "ext must be reported as disabled in mounts_info");

        // 验证探测结果即刻反映 ext 为 false
        let cfg_after_toggle = patcher::load_unified_config(u_str);
        assert!(!cfg_after_toggle.rime_ice_toggles.dict_ext, "dict_ext must be detected as false from rime_ice.dict.yaml");

        // 3. 模拟导入自定义词库并保存部署
        cfg.schemas = vec![patcher::SchemaItem {
            id: "rime_ice".to_string(),
            name: "雾凇拼音".to_string(),
            description: "雾凇拼音".to_string(),
            file_path: temp_dir.join("rime_ice.schema.yaml").to_string_lossy().to_string(),
            enabled: true,
        }];
        cfg.rime_ice_toggles.dict_ext = false;
        let custom_dir = temp_dir.join("custom_dicts");
        std::fs::create_dir_all(&custom_dir).unwrap();
        std::fs::write(custom_dir.join("test_dict.dict.yaml"), "# custom dict").unwrap();

        cfg.custom_dicts.push(patcher::CustomDictItem {
            name: "custom_dicts/test_dict".to_string(),
            file_name: "test_dict.dict.yaml".to_string(),
            full_path: custom_dir.join("test_dict.dict.yaml").to_string_lossy().to_string(),
            description: "测试自定义词库".to_string(),
            enabled: true,
            size_kb: 1.0,
        });

        patcher::save_unified_config(u_str, cfg.clone()).unwrap();

        // 4. 验证 rime_ice.extended.dict.yaml 的生成内容
        let ext_dict_content = std::fs::read_to_string(temp_dir.join("rime_ice.extended.dict.yaml")).unwrap();
        assert!(ext_dict_content.contains("cn_dicts/8105"), "extended dict must contain 8105");
        assert!(ext_dict_content.contains("cn_dicts/base"), "extended dict must contain base");
        assert!(ext_dict_content.contains("# - cn_dicts/ext"), "extended dict must contain commented out ext");
        assert!(!ext_dict_content.contains("\n  - cn_dicts/ext"), "extended dict must NOT contain enabled ext");
        assert!(ext_dict_content.contains("custom_dicts/test_dict"), "extended dict must contain custom dict");

        // 5. 验证保存后全量重读时：ext 保持关闭，custom dict 保持开启
        let reloaded_cfg = patcher::load_unified_config(u_str);
        assert!(!reloaded_cfg.rime_ice_toggles.dict_ext, "ext must remain disabled after saving and reloading");
        assert!(reloaded_cfg.rime_ice_toggles.dict_base, "base must remain enabled");
        let reloaded_custom = reloaded_cfg.custom_dicts.iter().find(|d| d.name == "custom_dicts/test_dict").unwrap();
        assert!(reloaded_custom.enabled, "custom dict must remain enabled");

        // 6. 在 extended 存在的情况下，通过 toggle_dict_mount_table 重新启用 ext
        let mounts_re_enabled = patcher::toggle_dict_mount_table(u_str, "rime_ice", "cn_dicts/ext", true).unwrap();
        let re_ext = mounts_re_enabled.mounted_tables.iter().find(|t| t.name == "cn_dicts/ext").unwrap();
        assert!(re_ext.enabled, "ext must be re-enabled");

        let ext_dict_synced = std::fs::read_to_string(temp_dir.join("rime_ice.extended.dict.yaml")).unwrap();
        assert!(ext_dict_synced.contains("cn_dicts/ext"), "extended dict must have ext restored");

        let rime_ice_synced = std::fs::read_to_string(temp_dir.join("rime_ice.dict.yaml")).unwrap();
        assert!(rime_ice_synced.contains("- cn_dicts/ext"), "rime_ice.dict.yaml must have - cn_dicts/ext restored");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_toggle_table_inline_comments_and_deduplication() {
        let temp_dir = std::env::temp_dir().join(format!("rimetune_comment_test_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let dict_file = temp_dir.join("test.dict.yaml");

        // 包含行内注释、历史损坏缺少 # 的注释、以及重复行
        let content = r#"# Rime dictionary
---
name: test
import_tables:
  - cn_dicts/8105       # 通用汉字
  - cn_dicts/base       # 基础词库
  - cn_dicts/base       # 基础词库重复行
  - cn_dicts/41448      大字表生僻字无井号
...
"#;
        std::fs::write(&dict_file, content).unwrap();

        // 1. 关闭 cn_dicts/base：行内注释必须保留 #，且重复行被清除
        let res = patcher::toggle_table_in_file(&dict_file, "cn_dicts/base", false);
        assert!(res.is_ok());
        let read1 = std::fs::read_to_string(&dict_file).unwrap();
        assert!(read1.contains("# - cn_dicts/base"), "must comment out base");
        assert!(read1.contains("# 基础词库"), "comment must retain its hash symbol");
        assert_eq!(read1.matches("cn_dicts/base").count(), 1, "duplicates must be eliminated");

        // 2. 重新开启 cn_dicts/base：行内注释必须保留 #
        let res2 = patcher::toggle_table_in_file(&dict_file, "cn_dicts/base", true);
        assert!(res2.is_ok());
        let read2 = std::fs::read_to_string(&dict_file).unwrap();
        assert!(read2.contains("- cn_dicts/base"), "must uncomment base");
        assert!(read2.contains("# 基础词库"), "comment must retain its hash symbol");
        assert_eq!(read2.matches("cn_dicts/base").count(), 1, "must remain deduplicated");

        // 3. 关闭 cn_dicts/41448：之前缺少 # 的注释应当被自愈
        let res3 = patcher::toggle_table_in_file(&dict_file, "cn_dicts/41448", false);
        assert!(res3.is_ok());
        let read3 = std::fs::read_to_string(&dict_file).unwrap();
        assert!(read3.contains("# - cn_dicts/41448"), "must comment out 41448");
        assert!(read3.contains("# 大字表生僻字无井号"), "healed comment must have hash symbol");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
