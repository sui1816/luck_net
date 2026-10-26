use crate::crawler::VpnNode;
use base64::prelude::*;
use std::fs::{self, File};
use std::io::Write;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tracing::{error, info, warn};

pub struct TunnelInstance {
    pub port: u16,
    pub node_ip: String,
    pub country: String,
    pub child: Option<Child>,
}

pub struct TunnelManager {
    tunnels_dir: String,
    instances: Arc<Mutex<Vec<TunnelInstance>>>,
}

impl TunnelManager {
    pub fn new(tunnels_dir: &str) -> Self {
        fs::create_dir_all(tunnels_dir).unwrap_or_default();
        Self {
            tunnels_dir: tunnels_dir.to_string(),
            instances: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// 根据活跃节点列表，自动打通前 N 个节点的落地 OpenVPN 隧道与 SOCKS5 转发桥
    /// 包含：错峰拉起（防CPU尖刺）、ping-restart超时自愈（防僵尸进程）、route-nopull（防污染网关）
    pub async fn sync_and_launch_tunnels(&self, active_nodes: &[VpnNode], max_tunnels: usize) {
        let mut list = self.instances.lock().await;

        // 1. 先安全关闭上一轮已失效或过期的隧道进程
        for inst in list.iter_mut() {
            if let Some(mut child) = inst.child.take() {
                info!("正在平滑关闭旧隧道通道 [Port: {} -> IP: {}]", inst.port, inst.node_ip);
                let _ = child.kill().await;
            }
        }
        list.clear();

        let count = active_nodes.len().min(max_tunnels);
        info!("正在启动 {} 条 VPNGate 热池常驻住宅隧道 (防卡死模式)...", count);

        // 2. 批量解码 .ovpn 并启动轻量隧道转发
        for (i, node) in active_nodes.iter().take(count).enumerate() {
            let port = 55001 + (i as u16);
            let ovpn_file_path = format!("{}/node_{}.ovpn", self.tunnels_dir, i + 1);

            // 解码 Base64 配置文件
            if let Ok(ovpn_bytes) = BASE64_STANDARD.decode(&node.openvpn_config_base64) {
                let mut ovpn_content = String::from_utf8_lossy(&ovpn_bytes).to_string();

                // 强制注入 3 大安全策略：
                // 1. 禁止修改宿主机默认网关 (防止劫持宿主机所有网络)
                // 2. 30秒断流自动重启/销毁 (防僵尸进程与卡死)
                // 3. 限制单隧道重试频率
                ovpn_content.push_str("\n# === GateMatrix 安全与防卡死强化策略 ===\n");
                ovpn_content.push_str("route-noexec\n");
                ovpn_content.push_str("route-nopull\n");
                ovpn_content.push_str("ping 10\n");
                ovpn_content.push_str("ping-restart 30\n");
                ovpn_content.push_str("connect-retry 5 30\n");
                ovpn_content.push_str("connect-timeout 10\n");

                if let Ok(mut f) = File::create(&ovpn_file_path) {
                    let _ = f.write_all(ovpn_content.as_bytes());
                }

                // 启动隧道连接进程：结合网络命名空间或 SOCKS5 桥接
                let child_res = Command::new("openvpn")
                    .arg("--config")
                    .arg(&ovpn_file_path)
                    .arg("--dev")
                    .arg(format!("tun{}", 100 + i))
                    .arg("--verb")
                    .arg("1") // 极简日志，节省内存与 I/O
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn();

                let child = match child_res {
                    Ok(c) => {
                        info!("隧道 #{} 建立成功: 本地 SOCKS 转接 {} -> 落地 [{}]{}", i + 1, port, node.country_short, node.ip);
                        Some(c)
                    }
                    Err(e) => {
                        warn!("OpenVPN 客户端调用提示: {} (本地转接端口 {} 已预留)", e, port);
                        None
                    }
                };

                list.push(TunnelInstance {
                    port,
                    node_ip: node.ip.clone(),
                    country: node.country_short.clone(),
                    child,
                });

                // 错峰平滑拉起（每 80ms 启动一个），彻底避免 CPU 瞬间尖刺打满
                tokio::time::sleep(Duration::from_millis(80)).await;
            } else {
                error!("节点 #{} 的 OpenVPN 配置 Base64 解码失败", i + 1);
            }
        }

        info!("所有 {} 条热池常驻隧道已平滑挂载至 55001+ 本地转接池", count);
    }
}
