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

        // 3. Fcitx5 环境：通过 D-Bus 接口通知 Fcitx5-Rime 重新部署，并重载配置
        let is_fcitx5 = user_dir.contains("fcitx5")
            || std::path::Path::new("/usr/bin/fcitx5").exists()
            || std::path::Path::new("/usr/bin/fcitx5-remote").exists();

        if is_fcitx5 {
            // 通过 busctl 调用 Fcitx5 接口通知 Rime 重新部署
            let busctl_status = Command::new("busctl")
                .args(["call", "--user", "org.fcitx.Fcitx5", "/controller", "org.fcitx.Fcitx.Controller1", "SetConfig", "sv", "fcitx://config/addon/rime/deploy", "s", ""])
                .status();

            let qdbus_status = if busctl_status.as_ref().map(|s| s.success()).unwrap_or(false) {
                busctl_status
            } else {
                Command::new("qdbus")
                    .args(["org.fcitx.Fcitx5", "/controller", "org.fcitx.Fcitx.Controller1.SetConfig", "fcitx://config/addon/rime/deploy", ""])
                    .status()
            };

            // 同时重载 fcitx5 守护进程
            let _ = Command::new("fcitx5-remote").arg("-r").status();

            let method_note = if rime_deployed {
                "rime_deployer + Fcitx5 D-Bus"
            } else if qdbus_status.as_ref().map(|s| s.success()).unwrap_or(false) {
                "Fcitx5 D-Bus 重新部署"
            } else {
                "Fcitx5 重载 (已清空构建缓存并更新时间戳)"
            };

            return DeployResult {
                success: true,
                message: format!("已成功触发 Fcitx5-Rime 重新部署与重载 ({})", method_note),
                deployer_used: method_note.to_string(),
            };
        }

        // 4. IBus 环境
        let is_ibus = user_dir.contains("ibus") || std::path::Path::new("/usr/bin/ibus").exists();
        if is_ibus {
            let _ = Command::new("ibus").arg("restart").status();
            return DeployResult {
                success: true,
                message: "已更新配置时间戳并重启 IBus-Rime 服务生效".to_string(),
                deployer_used: "ibus restart".to_string(),
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
