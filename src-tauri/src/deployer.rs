use serde::{Deserialize, Serialize};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use crate::detector::detect_weasel_paths;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DeployResult {
    pub success: bool,
    pub message: String,
    pub deployer_used: String,
}

// 全局互斥状态，防止并发多次调用 WeaselDeployer 导致死锁或文件争抢
static IS_DEPLOYING: AtomicBool = AtomicBool::new(false);

struct DeployGuard;
impl Drop for DeployGuard {
    fn drop(&mut self) {
        IS_DEPLOYING.store(false, Ordering::SeqCst);
    }
}

/// 执行 WeaselDeployer 命令（支持 /deploy 和 /sync 等参数），带互斥锁与超时保护
pub fn run_weasel_command(
    arg: &str,
    custom_deployer: Option<String>,
    success_msg: &str,
) -> DeployResult {
    // 1. 尝试获取互斥锁。如果已有部署任务正在进行，直接返回提示，避免重复执行争抢锁
    if IS_DEPLOYING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return DeployResult {
            success: false,
            message: "输入法部署程序正在后台执行中，请耐心等待完成，请勿频繁重复点击！".to_string(),
            deployer_used: String::new(),
        };
    }

    let _guard = DeployGuard;

    let (_, detected_user_dir, detected) = detect_weasel_paths();
    #[allow(unused_variables)]
    let user_dir = detected_user_dir.unwrap_or_default();
    let deployer_path = if let Some(p) = custom_deployer {
        if !p.trim().is_empty() && std::path::Path::new(&p).exists() {
            p
        } else {
            detected.unwrap_or_default()
        }
    } else {
        detected.unwrap_or_default()
    };

    #[cfg(target_os = "linux")]
    {
        // 1. 如果 user_dir 存在，先删除 build/default.yaml 并更新 installation.yaml 时间戳（确保触发缓存重新生成）
        if !user_dir.is_empty() {
            let u_path = std::path::Path::new(&user_dir);
            let inst_file = u_path.join("installation.yaml");
            if inst_file.exists() {
                let _ = std::fs::File::open(&inst_file).and_then(|f| f.set_modified(std::time::SystemTime::now()));
            } else {
                let _ = std::fs::write(&inst_file, "installation_id: \"rimetune\"\n");
            }
            let build_dir = u_path.join("build");
            let default_yaml = build_dir.join("default.yaml");
            if default_yaml.exists() {
                let _ = std::fs::remove_file(default_yaml);
            }

            // 自动检查并预编译 OpenCC 二进制字典（如 emoji.ocd2、others.ocd2 等）
            let opencc_dict_bin = ["/usr/bin/opencc_dict", "/usr/local/bin/opencc_dict"]
                .into_iter()
                .find(|p| std::path::Path::new(p).exists());
            if let Some(ocd_bin) = opencc_dict_bin {
                let opencc_dir = u_path.join("opencc");
                if opencc_dir.exists() {
                    if let Ok(entries) = std::fs::read_dir(&opencc_dir) {
                        for entry in entries.flatten() {
                            let p = entry.path();
                            if p.extension().and_then(|s| s.to_str()) == Some("txt") {
                                let stem = p.file_stem().unwrap_or_default().to_string_lossy();
                                let ocd2_path = opencc_dir.join(format!("{}.ocd2", stem));
                                let should_compile = if !ocd2_path.exists() {
                                    true
                                } else if let (Ok(m_txt), Ok(m_ocd2)) = (p.metadata(), ocd2_path.metadata()) {
                                    m_txt.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH) >
                                        m_ocd2.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH)
                                } else {
                                    false
                                };
                                if should_compile {
                                    let _ = Command::new(ocd_bin)
                                        .arg("-i").arg(&p)
                                        .arg("-o").arg(&ocd2_path)
                                        .arg("-f").arg("text")
                                        .arg("-t").arg("ocd2")
                                        .status();
                                }
                            }
                        }
                    }
                }
            }
        }

        // 2. 尝试执行 rime_deployer 进行真实全量编译
        let rime_deployer_bin = ["/usr/bin/rime_deployer", "/usr/local/bin/rime_deployer", "rime_deployer"]
            .into_iter()
            .find(|p| std::path::Path::new(p).exists());

        let mut rime_deployed = false;
        if let Some(rdb) = rime_deployer_bin {
            if !user_dir.is_empty() {
                let shared_dirs = ["/usr/share/rime-data", "/usr/local/share/rime-data", "/usr/share/rime"];
                let shared_dir = shared_dirs.into_iter().find(|d| std::path::Path::new(d).exists()).unwrap_or("/usr/share/rime-data");
                let build_dir = format!("{}/build", user_dir);
                let _ = std::fs::create_dir_all(&build_dir);

                let mut rcmd = Command::new(rdb);
                if arg == "/sync" {
                    rcmd.arg("--sync");
                } else {
                    rcmd.arg("--build")
                        .arg(&user_dir)
                        .arg(shared_dir)
                        .arg(&build_dir);
                }
                if let Ok(mut c) = rcmd.spawn() {
                    let _ = c.wait();
                    rime_deployed = true;
                }
            }
        }

        // 3. 判断当前激活的目标环境：优先根据用户配置目录，次之根据当前正在运行的守护进程
        let is_user_dir_ibus = user_dir.contains("ibus");
        let is_user_dir_fcitx5 = user_dir.contains("fcitx5");

        let (target_is_ibus, target_is_fcitx5) = if is_user_dir_ibus {
            (true, false)
        } else if is_user_dir_fcitx5 {
            (false, true)
        } else {
            let p_fcitx5 = Command::new("pgrep").arg("-x").arg("fcitx5").output().map(|o| o.status.success()).unwrap_or(false);
            let p_ibus = Command::new("pgrep").arg("-x").arg("ibus-daemon").output().map(|o| o.status.success()).unwrap_or(false);
            if p_fcitx5 {
                (false, true)
            } else if p_ibus {
                (true, false)
            } else if std::path::Path::new("/usr/bin/fcitx5").exists() || std::path::Path::new("/usr/bin/fcitx5-remote").exists() {
                (false, true)
            } else {
                (true, false)
            }
        };

        // Fcitx5 环境：通知部署并彻底重启 Fcitx5 服务以清空旧会话并使编译后的新配置即刻生效
        if target_is_fcitx5 {
            // 自动检查并解除 Fcitx5 顶层对 Shift_L 的快捷键劫持 (AltTriggerKeys)，让 Rime 的 ascii_composer 完全接管左右 Shift 动作
            if let Some(home) = std::env::var_os("HOME") {
                let fcitx_cfg_path = std::path::PathBuf::from(home).join(".config/fcitx5/config");
                if fcitx_cfg_path.exists() {
                    if let Ok(content) = std::fs::read_to_string(&fcitx_cfg_path) {
                        if content.contains("0=Shift_L") {
                            let updated = content.replace("0=Shift_L", "# 0=Shift_L");
                            let _ = std::fs::write(&fcitx_cfg_path, updated);
                        }
                    }
                }
            }

            // 1. 通过 D-Bus 接口通知 Fcitx5-Rime 执行部署指令
            let _ = Command::new("busctl")
                .args(["call", "--user", "org.fcitx.Fcitx5", "/controller", "org.fcitx.Fcitx.Controller1", "SetConfig", "sv", "fcitx://config/addon/rime/deploy", "s", ""])
                .status();

            // 2. 清理并彻底重启 Fcitx5 服务进程。
            // 关键作用：在 Wayland / KWin 等现代桌面下，D-Bus Restart 因 CanRestart 为 false 无法重载内存会话，
            // 只有干净退出并重新启动 fcitx5 -d，才能释放已占用的 userdb 锁、清空旧内存状态，并即刻加载最新的所有开关及按键！
            let _ = Command::new("fcitx5-remote").arg("-e").status();
            std::thread::sleep(std::time::Duration::from_millis(250));
            let _ = Command::new("pkill").arg("-9").arg("-x").arg("fcitx5").status();
            std::thread::sleep(std::time::Duration::from_millis(150));
            let _ = Command::new("fcitx5")
                .arg("-d")
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();

            return DeployResult {
                success: true,
                message: "已成功完成 Fcitx5-Rime 方案全量编译并重启 Fcitx5 服务，所有输入特性与按键即刻生效！".to_string(),
                deployer_used: "rime_deployer + Fcitx5 进程重启".to_string(),
            };
        }

        // IBus 环境
        if target_is_ibus {
            let _ = Command::new("ibus").arg("restart").status();
            return DeployResult {
                success: true,
                message: format!("已成功触发 IBus-Rime 部署与服务重启 ({})", if rime_deployed { "rime_deployer + ibus restart" } else { "ibus restart" }),
                deployer_used: if rime_deployed { "rime_deployer + ibus restart".to_string() } else { "ibus restart".to_string() },
            };
        }

        if rime_deployed {
            return DeployResult {
                success: true,
                message: "已通过 rime_deployer 完成输入法方案重新构建".to_string(),
                deployer_used: "rime_deployer".to_string(),
            };
        }
    }

    if deployer_path.is_empty() || !std::path::Path::new(&deployer_path).exists() {
        return DeployResult {
            success: false,
            message: "未找到输入法部署程序（WeaselDeployer/Squirrel/rime_deployer/fcitx5-remote），请检查输入法安装路径".to_string(),
            deployer_used: String::new(),
        };
    }

    // 2. 构造并启动子进程，适配不同操作系统和部署器程序
    let mut cmd = Command::new(&deployer_path);

    let is_fcitx5 = deployer_path.contains("fcitx5-remote");
    let is_rime_deployer = deployer_path.contains("rime_deployer");
    let is_squirrel = deployer_path.contains("Squirrel");

    if is_fcitx5 {
        cmd.arg("-r");
    } else if is_rime_deployer {
        if arg == "/sync" {
            cmd.arg("--sync");
        } else {
            cmd.arg("--build");
        }
    } else if is_squirrel {
        cmd.arg("--reload");
    } else {
        cmd.arg(arg);
    }

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return DeployResult {
                success: false,
                message: format!("无法启动部署程序进程: {}", e),
                deployer_used: deployer_path,
            };
        }
    };

    // 3. 超时循环等待（小狼毫部署可能需重新编译词库，设置 45 秒超时上限），绝不永久阻塞主线程
    let start = Instant::now();
    let timeout = Duration::from_secs(45);
    let exit_status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    return DeployResult {
                        success: false,
                        message: "输入法部署超时（已等待超过 45 秒）。可能因词库或配置正在被独占占用，已自动停止等待。".to_string(),
                        deployer_used: deployer_path,
                    };
                }
                std::thread::sleep(Duration::from_millis(150));
            }
            Err(e) => {
                return DeployResult {
                    success: false,
                    message: format!("等待部署进程退出时出错: {}", e),
                    deployer_used: deployer_path,
                };
            }
        }
    };

    if exit_status.success() {
        DeployResult {
            success: true,
            message: success_msg.to_string(),
            deployer_used: deployer_path,
        }
    } else {
        DeployResult {
            success: false,
            message: format!("部署程序返回非零退出代码: {}", exit_status),
            deployer_used: deployer_path,
        }
    }
}

pub fn trigger_deploy(custom_deployer: Option<String>) -> DeployResult {
    run_weasel_command(
        "/deploy",
        custom_deployer,
        "重新部署指令已成功发送给输入法，配置与词库已刷新！",
    )
}

/// 在 Linux 环境下一键安装 Rime 扩展插件及 OpenCC 工具 (调起 pkexec 授权执行)
pub fn install_linux_plugins() -> Result<String, String> {
    #[cfg(target_os = "linux")]
    {
        let (cmd, args) = if std::path::Path::new("/usr/bin/dnf").exists() {
            ("/usr/bin/pkexec", vec!["dnf", "install", "-y", "librime-lua", "librime-octagram", "opencc", "opencc-tools"])
        } else if std::path::Path::new("/usr/bin/apt-get").exists() {
            ("/usr/bin/pkexec", vec!["apt-get", "install", "-y", "librime-plugin-lua", "librime-plugin-octagram", "opencc"])
        } else if std::path::Path::new("/usr/bin/pacman").exists() {
            ("/usr/bin/pkexec", vec!["pacman", "-S", "--noconfirm", "librime-lua", "opencc"])
        } else if std::path::Path::new("/usr/bin/zypper").exists() {
            ("/usr/bin/pkexec", vec!["zypper", "install", "-y", "librime-lua", "librime-octagram", "opencc"])
        } else {
            return Err("未识别的 Linux 发行版包管理器，请手动在终端执行安装。".to_string());
        };

        let status = std::process::Command::new(cmd)
            .args(&args)
            .status()
            .map_err(|e| format!("启动授权安装程序失败: {e}"))?;

        if status.success() {
            // 安装完成后自动重载 fcitx5
            let _ = std::process::Command::new("busctl")
                .args(["call", "--user", "org.fcitx.Fcitx5", "/controller", "org.fcitx.Fcitx.Controller1", "Restart"])
                .status();
            Ok("安装成功！已自动重启输入法服务。".to_string())
        } else {
            Err(format!("安装未成功完成或用户取消授权 (退出状态: {status:?})"))
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        Ok("当前非 Linux 系统，无需安装系统级插件。".to_string())
    }
}
