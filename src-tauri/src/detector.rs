use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[cfg(windows)]
use winreg::enums::*;
#[cfg(windows)]
use winreg::RegKey;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GrammarModelItem {
    pub name: String,
    pub filename: String,
    pub size_bytes: u64,
    pub size_formatted: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EnvironmentStatus {
    pub platform: String,
    pub host_id: String,
    pub host_name: String,
    pub host_display_name: String,
    pub custom_filename: String,
    pub weasel_installed: bool,
    pub weasel_version: String,
    pub weasel_root: String,
    pub deployer_path: String,
    pub rime_user_dir: String,
    pub is_rime_ice_detected: bool,
    pub active_schema_id: String,
    pub active_schema_name: String,
    pub patch_files_found: Vec<String>,
    pub total_phrases_count: usize,
    pub recommended_phrase_file: String,
    pub is_double_pinyin: bool,
    pub available_grammar_models: Vec<GrammarModelItem>,
    pub is_lua_available: bool,
    pub is_octagram_available: bool,
    pub is_opencc_available: bool,
    pub linux_pkg_manager: String,
    pub linux_install_command: String,
}

pub fn detect_weasel_paths() -> (Option<String>, Option<String>, Option<String>) {
    let mut weasel_root: Option<String> = None;
    let mut user_dir: Option<String> = None;

    #[cfg(windows)]
    {
        let subkeys = [
            (HKEY_CURRENT_USER, r"Software\Rime\Weasel"),
            (HKEY_LOCAL_MACHINE, r"Software\WOW6432Node\Rime\Weasel"),
            (HKEY_LOCAL_MACHINE, r"Software\Rime\Weasel"),
        ];

        for (hkey, subkey) in &subkeys {
            if let Ok(key) = RegKey::predef(*hkey).open_subkey(subkey) {
                if weasel_root.is_none() {
                    if let Ok(val) = key.get_value::<String, _>("WeaselRoot") {
                        if !val.trim().is_empty() && Path::new(&val).exists() {
                            weasel_root = Some(val);
                        }
                    }
                }
                if weasel_root.is_none() {
                    if let Ok(val) = key.get_value::<String, _>("InstallDir") {
                        if !val.trim().is_empty() && Path::new(&val).exists() {
                            weasel_root = Some(val);
                        }
                    }
                }
                if user_dir.is_none() {
                    if let Ok(val) = key.get_value::<String, _>("RimeUserDir") {
                        if !val.trim().is_empty() && Path::new(&val).exists() {
                            user_dir = Some(val);
                        }
                    }
                }
            }
        }

        if user_dir.is_none() {
            if let Ok(appdata) = std::env::var("APPDATA") {
                let p = PathBuf::from(appdata).join("Rime");
                if p.exists() {
                    user_dir = Some(p.to_string_lossy().to_string());
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            let rime_dir = PathBuf::from(&home).join("Library/Rime");
            if rime_dir.exists() {
                user_dir = Some(rime_dir.to_string_lossy().to_string());
            }
        }
        let squirrel_app = Path::new("/Library/Input Methods/Squirrel.app");
        if squirrel_app.exists() {
            weasel_root = Some(squirrel_app.to_string_lossy().to_string());
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(home) = std::env::var("HOME") {
            let candidates = [
                PathBuf::from(&home).join(".local/share/fcitx5/rime"),
                PathBuf::from(&home).join(".config/fcitx5/rime"),
                PathBuf::from(&home).join(".config/ibus/rime"),
                PathBuf::from(&home).join(".local/share/rime"),
                PathBuf::from(&home).join(".config/rime"),
            ];
            for c in &candidates {
                if c.exists() {
                    user_dir = Some(c.to_string_lossy().to_string());
                    break;
                }
            }
            if user_dir.is_none() {
                user_dir = Some(PathBuf::from(&home).join(".local/share/fcitx5/rime").to_string_lossy().to_string());
            }
        }
    }

    let mut deployer: Option<String> = None;

    #[cfg(windows)]
    {
        if let Some(ref root) = weasel_root {
            let p = PathBuf::from(root).join("WeaselDeployer.exe");
            if p.exists() {
                deployer = Some(p.to_string_lossy().to_string());
            }
        }

        if deployer.is_none() {
            let candidates = [
                r"C:\Program Files\Rime\weasel-0.17.0\WeaselDeployer.exe",
                r"C:\Program Files\Rime\weasel-0.16.3\WeaselDeployer.exe",
                r"C:\Program Files\Rime\weasel-0.16.2\WeaselDeployer.exe",
                r"C:\Program Files\Rime\weasel-0.16.1\WeaselDeployer.exe",
                r"C:\Program Files\Rime\weasel-0.16.0\WeaselDeployer.exe",
                r"C:\Program Files (x86)\Rime\weasel-0.16.3\WeaselDeployer.exe",
                r"C:\Program Files (x86)\Rime\weasel-0.16.2\WeaselDeployer.exe",
                r"C:\Program Files (x86)\Rime\weasel-0.16.1\WeaselDeployer.exe",
                r"C:\Program Files (x86)\Rime\weasel-0.16.0\WeaselDeployer.exe",
                r"C:\Program Files\Rime\WeaselDeployer.exe",
                r"C:\Program Files (x86)\Rime\WeaselDeployer.exe",
            ];
            for c in &candidates {
                if Path::new(c).exists() {
                    deployer = Some(c.to_string());
                    if weasel_root.is_none() {
                        if let Some(parent) = Path::new(c).parent() {
                            weasel_root = Some(parent.to_string_lossy().to_string());
                        }
                    }
                    break;
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        let squirrel_bin = Path::new("/Library/Input Methods/Squirrel.app/Contents/MacOS/Squirrel");
        if squirrel_bin.exists() {
            deployer = Some(squirrel_bin.to_string_lossy().to_string());
        }
    }

    #[cfg(target_os = "linux")]
    {
        for bin in &["/usr/bin/rime_deployer", "/usr/bin/fcitx5-remote"] {
            if Path::new(bin).exists() {
                deployer = Some(bin.to_string());
                break;
            }
        }
    }

    (weasel_root, user_dir, deployer)
}

pub fn inspect_environment() -> EnvironmentStatus {
    let (weasel_root_opt, user_dir_opt, deployer_opt) = detect_weasel_paths();
    let weasel_installed = deployer_opt.is_some();
    let weasel_root = weasel_root_opt.clone().unwrap_or_default();
    let deployer_path = deployer_opt.unwrap_or_default();
    let rime_user_dir = user_dir_opt.unwrap_or_else(|| {
        #[cfg(windows)]
        {
            std::env::var("APPDATA")
                .map(|a| PathBuf::from(a).join("Rime").to_string_lossy().to_string())
                .unwrap_or_default()
        }
        #[cfg(target_os = "macos")]
        {
            std::env::var("HOME")
                .map(|h| PathBuf::from(h).join("Library/Rime").to_string_lossy().to_string())
                .unwrap_or_default()
        }
        #[cfg(target_os = "linux")]
        {
            std::env::var("HOME")
                .map(|h| PathBuf::from(h).join(".local/share/fcitx5/rime").to_string_lossy().to_string())
                .unwrap_or_default()
        }
        #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
        {
            String::new()
        }
    });

    #[cfg(windows)]
    let platform = "windows".to_string();
    #[cfg(target_os = "macos")]
    let platform = "macos".to_string();
    #[cfg(target_os = "linux")]
    let platform = "linux".to_string();
    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    let platform = "unknown".to_string();

    #[cfg(windows)]
    let (host_id, host_name, host_display_name, custom_filename) = (
        "weasel".to_string(),
        "小狼毫 (Weasel)".to_string(),
        "小狼毫".to_string(),
        "weasel.custom.yaml".to_string(),
    );

    #[cfg(target_os = "macos")]
    let (host_id, host_name, host_display_name, custom_filename) = (
        "squirrel".to_string(),
        "鼠须管 (Squirrel)".to_string(),
        "鼠须管".to_string(),
        "squirrel.custom.yaml".to_string(),
    );

    #[cfg(target_os = "linux")]
    let (host_id, host_name, host_display_name, custom_filename) = {
        let is_user_dir_ibus = rime_user_dir.contains("ibus");
        let is_user_dir_fcitx5 = rime_user_dir.contains("fcitx5");

        let (is_ibus, is_fcitx5) = if is_user_dir_ibus {
            (true, false)
        } else if is_user_dir_fcitx5 {
            (false, true)
        } else {
            // 用户目录无显式标识时，优先检测当前正在运行的输入法守护进程
            let is_fcitx5_running = Path::new("/usr/bin/fcitx5-remote").exists()
                && std::process::Command::new("pgrep").arg("-x").arg("fcitx5").output().map(|o| o.status.success()).unwrap_or(false);
            let is_ibus_running = Path::new("/usr/bin/ibus").exists()
                && std::process::Command::new("pgrep").arg("-x").arg("ibus-daemon").output().map(|o| o.status.success()).unwrap_or(false);

            if is_fcitx5_running {
                (false, true)
            } else if is_ibus_running {
                (true, false)
            } else if Path::new("/usr/bin/fcitx5").exists() || Path::new("/usr/bin/fcitx5-remote").exists() {
                (false, true)
            } else if Path::new("/usr/bin/ibus").exists() {
                (true, false)
            } else {
                (false, false)
            }
        };

        if is_ibus {
            (
                "ibus".to_string(),
                "IBus-Rime".to_string(),
                "IBus-Rime".to_string(),
                "default.custom.yaml".to_string(),
            )
        } else if is_fcitx5 {
            (
                "fcitx5".to_string(),
                "Fcitx5-Rime".to_string(),
                "Fcitx5-Rime".to_string(),
                "default.custom.yaml".to_string(),
            )
        } else {
            (
                "rime".to_string(),
                "中州韵 (Rime)".to_string(),
                "中州韵".to_string(),
                "default.custom.yaml".to_string(),
            )
        }
    };

    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    let (host_id, host_name, host_display_name, custom_filename) = (
        "rime".to_string(),
        "中州韵 (Rime)".to_string(),
        "中州韵".to_string(),
        "default.custom.yaml".to_string(),
    );

    let mut weasel_version = String::new();
    if !weasel_root.is_empty() {
        let p = Path::new(&weasel_root);
        if let Some(fname) = p.file_name() {
            let s = fname.to_string_lossy();
            if s.starts_with("weasel-") {
                weasel_version = s.replace("weasel-", "");
            }
        }
    }
    if weasel_version.is_empty() && weasel_installed {
        #[cfg(windows)]
        {
            weasel_version = "0.17.x".to_string();
        }
        #[cfg(target_os = "macos")]
        {
            weasel_version = "Squirrel".to_string();
        }
        #[cfg(target_os = "linux")]
        {
            weasel_version = host_display_name.clone();
        }
    }

    let user_path = Path::new(&rime_user_dir);

    // 收集所有可能的数据与方案目录（用户目录 + 宿主共享程序目录）
    let mut data_dirs = vec![user_path.to_path_buf()];
    if let Some(ref wr) = weasel_root_opt {
        let p = Path::new(wr).join("data");
        if p.exists() && !data_dirs.contains(&p) {
            data_dirs.push(p);
        }
    }
    #[cfg(target_os = "linux")]
    {
        for dir in &["/usr/share/rime-data", "/usr/local/share/rime-data", "/usr/share/rime"] {
            let p = PathBuf::from(dir);
            if p.exists() && !data_dirs.contains(&p) {
                data_dirs.push(p);
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        let p = PathBuf::from("/Library/Input Methods/Squirrel.app/Contents/SharedSupport");
        if p.exists() && !data_dirs.contains(&p) {
            data_dirs.push(p);
        }
    }
    #[cfg(windows)]
    {
        for fallback in &[
            r"C:\Program Files\Rime\weasel-0.17.0\data",
            r"C:\Program Files\Rime\weasel\data",
            r"C:\Program Files (x86)\Rime\weasel\data",
        ] {
            let fb_path = PathBuf::from(fallback);
            if fb_path.exists() && !data_dirs.contains(&fb_path) {
                data_dirs.push(fb_path);
            }
        }
    }

    // 探测当前是否存在现代增强型方案 (雾凇/白霜/薄荷/万象)
    let is_rime_ice_detected = data_dirs.iter().any(|d| {
        d.join("rime_ice.schema.yaml").exists()
            || d.join("rime_frost.schema.yaml").exists()
            || d.join("rime_mint.schema.yaml").exists()
            || d.join("wanxiang.schema.yaml").exists()
    });

    // 查找已存在的 custom.yaml 补丁文件
    let mut patch_files_found = Vec::new();
    if user_path.exists() {
        if let Ok(entries) = std::fs::read_dir(user_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(name) = path.file_name() {
                    let s = name.to_string_lossy();
                    if s.ends_with(".custom.yaml") {
                        patch_files_found.push(s.to_string());
                    }
                }
            }
        }
    }

    // 探测当前激活的输入方案
    let mut detected_id_opt: Option<String> = None;

    // 0. 先尝试读取 default.custom.yaml 中的用户显式设定的 schema_list
    let mut custom_schema_list: Vec<String> = Vec::new();
    let default_custom = user_path.join("default.custom.yaml");
    if default_custom.exists() {
        if let Ok(content) = std::fs::read_to_string(&default_custom) {
            if let Ok(val) = serde_yaml::from_str::<serde_yaml::Value>(&content) {
                if let Some(patch) = val.get("patch") {
                    if let Some(schema_list) = patch.get("schema_list").and_then(|l| l.as_sequence()) {
                        for item in schema_list {
                            if let Some(s) = item.get("schema").and_then(|s| s.as_str()) {
                                custom_schema_list.push(s.to_string());
                            }
                        }
                    } else if let Some(map) = patch.as_mapping() {
                        for (k, v) in map {
                            if let Some(k_str) = k.as_str() {
                                if k_str.contains("schema_list") && k_str.contains("@0") {
                                    if let Some(s) = v.get("schema").and_then(|s| s.as_str()) {
                                        custom_schema_list.push(s.to_string());
                                        break;
                                    } else if let Some(s) = v.as_str() {
                                        custom_schema_list.push(s.to_string());
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 1. 最高优先级：从 user.yaml 读取运行时真实激活的输入方案 (previously_selected_schema)
    let user_yaml = user_path.join("user.yaml");
    if user_yaml.exists() {
        if let Ok(content) = std::fs::read_to_string(&user_yaml) {
            if let Ok(val) = serde_yaml::from_str::<serde_yaml::Value>(&content) {
                if let Some(prev) = val.get("var").and_then(|v| v.get("previously_selected_schema")).and_then(|s| s.as_str()) {
                    let trimmed = prev.trim();
                    if !trimmed.is_empty() {
                        let in_custom = custom_schema_list.is_empty() || custom_schema_list.iter().any(|id| id == trimmed);
                        if in_custom {
                            let exists = data_dirs.iter().any(|d| d.join(format!("{}.schema.yaml", trimmed)).exists());
                            if exists {
                                detected_id_opt = Some(trimmed.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    // 2. 其次从 default.custom.yaml 获取用户当前配置的第一方案
    if detected_id_opt.is_none() && !custom_schema_list.is_empty() {
        if let Some(first) = custom_schema_list.first() {
            detected_id_opt = Some(first.clone());
        }
    }

    // 2. 其次从 user_dir/default.yaml 获取默认第一方案
    if detected_id_opt.is_none() {
        let default_yaml = user_path.join("default.yaml");
        if default_yaml.exists() {
            if let Ok(content) = std::fs::read_to_string(&default_yaml) {
                if let Ok(val) = serde_yaml::from_str::<serde_yaml::Value>(&content) {
                    if let Some(schema_list) = val.get("schema_list").and_then(|l| l.as_sequence()) {
                        if let Some(first) = schema_list.first() {
                            if let Some(schema) = first.get("schema").and_then(|s| s.as_str()) {
                                detected_id_opt = Some(schema.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    // 2.5 再次从 build/default.yaml（构建部署产物中的 schema_list 反映部署后的真实激活状态）
    if detected_id_opt.is_none() {
        let build_default = user_path.join("build").join("default.yaml");
        if build_default.exists() {
            if let Ok(content) = std::fs::read_to_string(&build_default) {
                if let Ok(val) = serde_yaml::from_str::<serde_yaml::Value>(&content) {
                    if let Some(schema_list) = val.get("schema_list").and_then(|l| l.as_sequence()) {
                        if let Some(first) = schema_list.first() {
                            if let Some(schema) = first.get("schema").and_then(|s| s.as_str()) {
                                detected_id_opt = Some(schema.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    // 2.6 从系统预置 shared data_dirs 的 default.yaml 获取默认激活方案
    if detected_id_opt.is_none() {
        for dir in &data_dirs {
            let shared_default = dir.join("default.yaml");
            if shared_default.exists() {
                if let Ok(content) = std::fs::read_to_string(&shared_default) {
                    if let Ok(val) = serde_yaml::from_str::<serde_yaml::Value>(&content) {
                        if let Some(schema_list) = val.get("schema_list").and_then(|l| l.as_sequence()) {
                            if let Some(first) = schema_list.first() {
                                if let Some(schema) = first.get("schema").and_then(|s| s.as_str()) {
                                    detected_id_opt = Some(schema.to_string());
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 3. 再次从用户目录直接存在的方案文件推断（排除反查辅码组件）
    if detected_id_opt.is_none() {
        if let Ok(entries) = std::fs::read_dir(user_path) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() {
                    if let Some(fname) = p.file_name().and_then(|f| f.to_str()) {
                        if fname.ends_with(".schema.yaml") {
                            let id = fname.replace(".schema.yaml", "");
                            if id != "radical_pinyin" && id != "wanxiang_reverse" {
                                detected_id_opt = Some(id);
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    // 4. 再次从系统预置 shared data_dirs 直接存在的方案文件推断
    if detected_id_opt.is_none() {
        for dir in &data_dirs {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_file() {
                        if let Some(fname) = p.file_name().and_then(|f| f.to_str()) {
                            if fname.ends_with(".schema.yaml") {
                                let id = fname.replace(".schema.yaml", "");
                                if id != "radical_pinyin" && id != "wanxiang_reverse" {
                                    detected_id_opt = Some(id);
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            if detected_id_opt.is_some() {
                break;
            }
        }
    }

    // 兜底回退：官方标准经典朙月拼音 (luna_pinyin)
    let active_schema_id = detected_id_opt.unwrap_or_else(|| "luna_pinyin".to_string());

    // 解析方案展示名称：优先从目标方案文件读 schema/name
    let mut active_schema_name = String::new();
    for dir in &data_dirs {
        let schema_file = dir.join(format!("{}.schema.yaml", active_schema_id));
        if schema_file.exists() {
            if let Ok(content) = std::fs::read_to_string(&schema_file) {
                if let Ok(val) = serde_yaml::from_str::<serde_yaml::Value>(&content) {
                    if let Some(n) = val.get("schema").and_then(|s| s.get("name")).and_then(|v| v.as_str()) {
                        active_schema_name = n.to_string();
                        break;
                    }
                }
            }
        }
    }

    if active_schema_name.is_empty() {
        active_schema_name = match active_schema_id.as_str() {
            "rime_frost" => "白霜拼音".to_string(),
            "rime_frost_double_pinyin" => "白霜·自然码双拼".to_string(),
            "rime_frost_double_pinyin_flypy" => "白霜·小鹤双拼".to_string(),
            "rime_frost_double_pinyin_mspy" => "白霜·微软双拼".to_string(),
            "rime_frost_double_pinyin_sogou" => "白霜·搜狗双拼".to_string(),
            "rime_frost_double_pinyin_ziguang" => "白霜·紫光双拼".to_string(),
            "rime_frost_double_pinyin_abc" => "白霜·智能ABC双拼".to_string(),
            "rime_frost_wubi86" => "白霜·五笔86".to_string(),
            "rime_frost_moqi_single_xh" => "白霜·墨奇音形".to_string(),
            "rime_mint" => "薄荷拼音".to_string(),
            "rime_mint_flypy" => "薄荷·小鹤双拼".to_string(),
            "wanxiang" => "万象拼音".to_string(),
            "wanxiang_t9" => "万象·九键拼音".to_string(),
            "wanxiang_t9i" => "万象·九键拼音i".to_string(),
            "wanxiang_english" => "万象·英文输入".to_string(),
            "rime_ice" => "雾凇拼音".to_string(),
            "double_pinyin_flypy" => "小鹤双拼".to_string(),
            "double_pinyin" => "自然码双拼".to_string(),
            "double_pinyin_mspy" => "微软双拼".to_string(),
            "melt_eng" => "英文输入".to_string(),
            "radical_pinyin" => "部件拆字".to_string(),
            "luna_pinyin" => "朙月拼音".to_string(),
            "luna_pinyin_simp" => "朙月拼音·简化字".to_string(),
            "luna_pinyin_fluency" => "朙月拼音·語句流".to_string(),
            "bopomofo" => "注音".to_string(),
            "bopomofo_tw" => "注音·臺灣正體".to_string(),
            "cangjie5" => "倉頡五代".to_string(),
            "stroke" => "五筆畫".to_string(),
            "terra_pinyin" => "地球拼音".to_string(),
            "wubi98_mint" => "五笔98·薄荷".to_string(),
            "wubi86_jidian" => "极点五笔86".to_string(),
            _ => active_schema_id.clone(),
        };
    }

    // 智能识别双拼方案与推荐的自定义短语文件
    let is_double_pinyin = crate::phrases::is_schema_double_pinyin(&active_schema_id, &rime_user_dir);
    let recommended_phrase_file = crate::phrases::get_recommended_phrase_file(&rime_user_dir, &active_schema_id);

    // 统计推荐短语文件的词条数（如果推荐文件不存在，尝试回退统计另一个存在的文件）
    let target_phrase_path = user_path.join(&recommended_phrase_file);
    let total_phrases_count = if target_phrase_path.exists() {
        crate::phrases::count_phrase_file_lines(&target_phrase_path)
    } else {
        let fallback_file = if recommended_phrase_file == "custom_phrase_double.txt" {
            "custom_phrase.txt"
        } else {
            "custom_phrase_double.txt"
        };
        crate::phrases::count_phrase_file_lines(&user_path.join(fallback_file))
    };

    let available_grammar_models = scan_grammar_models(&user_path);

    let (is_lua_available, is_octagram_available, is_opencc_available, linux_pkg_manager, linux_install_command) = {
        #[cfg(target_os = "linux")]
        {
            let plugin_dirs = [
                "/usr/lib64/rime-plugins",
                "/usr/lib/x86_64-linux-gnu/rime-plugins",
                "/usr/lib/rime-plugins",
                "/usr/local/lib/rime-plugins",
            ];
            let has_lua = plugin_dirs.iter().any(|d| Path::new(d).join("librime-lua.so").exists());
            let has_octa = plugin_dirs.iter().any(|d| Path::new(d).join("librime-octagram.so").exists());
            let has_opencc = Path::new("/usr/bin/opencc_dict").exists() || Path::new("/usr/local/bin/opencc_dict").exists();

            let (pkg_mgr, cmd) = if Path::new("/usr/bin/dnf").exists() {
                ("dnf".to_string(), "sudo dnf install -y librime-lua librime-octagram opencc opencc-tools".to_string())
            } else if Path::new("/usr/bin/apt-get").exists() || Path::new("/usr/bin/apt").exists() {
                ("apt".to_string(), "sudo apt update && sudo apt install -y librime-plugin-lua librime-plugin-octagram opencc".to_string())
            } else if Path::new("/usr/bin/pacman").exists() {
                ("pacman".to_string(), "sudo pacman -S --noconfirm librime-lua opencc".to_string())
            } else if Path::new("/usr/bin/zypper").exists() {
                ("zypper".to_string(), "sudo zypper install -y librime-lua librime-octagram opencc".to_string())
            } else {
                ("unknown".to_string(), "sudo dnf install -y librime-lua librime-octagram opencc opencc-tools".to_string())
            };

            (has_lua, has_octa, has_opencc, pkg_mgr, cmd)
        }
        #[cfg(not(target_os = "linux"))]
        {
            (true, true, true, String::new(), String::new())
        }
    };

    EnvironmentStatus {
        platform,
        host_id,
        host_name,
        host_display_name,
        custom_filename,
        weasel_installed,
        weasel_version,
        weasel_root,
        deployer_path,
        rime_user_dir,
        is_rime_ice_detected,
        active_schema_id,
        active_schema_name,
        patch_files_found,
        total_phrases_count,
        recommended_phrase_file,
        is_double_pinyin,
        available_grammar_models,
        is_lua_available,
        is_octagram_available,
        is_opencc_available,
        linux_pkg_manager,
        linux_install_command,
    }
}

pub fn scan_grammar_models(user_path: &Path) -> Vec<GrammarModelItem> {
    let mut models = Vec::new();
    if !user_path.exists() {
        return models;
    }

    if let Ok(entries) = std::fs::read_dir(user_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                    if ext.eq_ignore_ascii_case("gram") {
                        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                            let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or_default().to_string();
                            let size_bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
                            let size_formatted = if size_bytes >= 1024 * 1024 {
                                format!("{:.1} MB", size_bytes as f64 / (1024.0 * 1024.0))
                            } else if size_bytes >= 1024 {
                                format!("{:.1} KB", size_bytes as f64 / 1024.0)
                            } else {
                                format!("{} B", size_bytes)
                            };

                            models.push(GrammarModelItem {
                                name: stem.to_string(),
                                filename,
                                size_bytes,
                                size_formatted,
                            });
                        }
                    }
                }
            }
        }
    }

    // 排序：万象模型排前面，随后按名称字母排序
    models.sort_by(|a, b| {
        let a_wx = a.name.contains("wanxiang");
        let b_wx = b.name.contains("wanxiang");
        match (a_wx, b_wx) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.cmp(&b.name),
        }
    });

    models
}

