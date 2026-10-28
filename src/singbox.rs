use crate::scheduler::OUTBOUND_START_PORT;
use serde_json::{json, Value};
use std::fs::File;
use std::io::Write;
use std::path::Path;
use tracing::info;

pub struct SingBoxConfigManager {
    config_path: String,
    domain: String,
    reality_port: u16,
    hy2_port: u16,
    reality_private_key: String,
    reality_short_id: String,
    reality_server_name: String,
    cert_path: String,
    key_path: String,
    sub_token: String,
}

impl SingBoxConfigManager {
    pub fn new(
        config_path: &str,
        domain: &str,
        reality_port: u16,
        hy2_port: u16,
        reality_private_key: &str,
        reality_short_id: &str,
        reality_server_name: &str,
        cert_path: &str,
        key_path: &str,
        sub_token: &str,
    ) -> Self {
        Self {
            config_path: config_path.to_string(),
            domain: domain.to_string(),
            reality_port,
            hy2_port,
            reality_private_key: reality_private_key.to_string(),
            reality_short_id: reality_short_id.to_string(),
            reality_server_name: reality_server_name.to_string(),
            cert_path: cert_path.to_string(),
            key_path: key_path.to_string(),
            sub_token: sub_token.to_string(),
        }
    }

    /// 根据当前有效节点数，生成 VLESS-Reality + Hysteria 2 双栈多用户分流配置
    pub fn generate_and_save(&self, active_count: usize) -> Result<(), anyhow::Error> {
        info!(
            "正在生成双栈入站配置: Reality:TCP {}, Hy2:UDP {} (总用户/出口数: {})...",
            self.reality_port, self.hy2_port, active_count
        );

        let total_users = active_count.clamp(1, 5000);
        let mut reality_users = Vec::new();
        let mut hy2_users = Vec::new();

        // 提取 sub_token 的前 8 位和前 12 位作为盐值，生成当前部署独有的强随机 UUID
        let token_clean: String = self.sub_token.chars().filter(|c| c.is_ascii_hexdigit()).collect();
        let prefix = if token_clean.len() >= 8 { &token_clean[..8] } else { "a1b2c3d4" };
        let mid = if token_clean.len() >= 12 { &token_clean[8..12] } else { "e5f6" };

        // 1. 直连用户 (user_direct)
        let direct_uuid = format!("{}-{}-4000-8000-000000000000", prefix, mid);
        let direct_pass = format!("pass_{}_direct", &self.sub_token[..8.min(self.sub_token.len())]);

        reality_users.push(json!({
            "name": "user_direct",
            "uuid": direct_uuid
        }));
        hy2_users.push(json!({
            "name": "user_direct",
            "password": direct_pass
        }));

        // 2. 批量生成 1 ~ N 个住宅出口对应的专属随机用户
        for i in 1..=total_users {
            let user_name = format!("user_node_{}", i);
            // 结合部署 token 生成全网唯一的私有 UUID，杜绝公开碰撞
            let uuid = format!("{}-{}-4000-8000-{:012x}", prefix, mid, i);
            let password = format!("pass_{}_{:04}", &self.sub_token[..8.min(self.sub_token.len())], i);

            reality_users.push(json!({
                "name": user_name,
                "uuid": uuid
            }));

            hy2_users.push(json!({
                "name": user_name,
                "password": password
            }));
        }

        // 3. 构建 Inbounds 入站列表
        let mut inbounds = Vec::new();

        // (1) VLESS-Reality 高位端口入站 (TCP 8443 等, 同时支持微软与苹果大厂伪装)
        inbounds.push(json!({
            "type": "vless",
            "tag": "in_vless_reality",
            "listen": "::",
            "listen_port": self.reality_port,
            "users": reality_users,
            "tls": {
                "enabled": true,
                "server_name": "www.microsoft.com",
                "reality": {
                    "enabled": true,
                    "handshake": {
                        "server": "www.microsoft.com",
                        "server_port": 443
                    },
                    "private_key": self.reality_private_key,
                    "short_id": [self.reality_short_id]
                }
            }
        }));

        // (2) Hysteria 2 入站 (UDP 443, 绑定域名, 抗弱网丢包极速)
        let mut hy2_inbound = json!({
            "type": "hysteria2",
            "tag": "in_hy2",
            "listen": "::",
            "listen_port": self.hy2_port,
            "users": hy2_users,
            "tls": {
                "enabled": true,
                "server_name": self.domain
            }
        });

        // 如果配置了证书路径则挂载证书，否则使用内存自签证书
        if Path::new(&self.cert_path).exists() && Path::new(&self.key_path).exists() {
            hy2_inbound["tls"]["certificate_path"] = json!(self.cert_path);
            hy2_inbound["tls"]["key_path"] = json!(self.key_path);
        }

        inbounds.push(hy2_inbound);

        // 4. 构建 Outbounds 出站列表
        let mut outbounds = Vec::new();
        // 原生直连出站
        outbounds.push(json!({
            "type": "direct",
            "tag": "out_direct"
        }));

        // 内部 SOCKS5 转发出口池 (55001 ~ 55000+N)
        for i in 1..=total_users {
            let out_port = OUTBOUND_START_PORT + (i as u16) - 1;
            outbounds.push(json!({
                "type": "socks",
                "tag": format!("out_socks_{}", out_port),
                "server": "127.0.0.1",
                "server_port": out_port
            }));
        }

        // 5. 构建 Route 路由分流规则 (基于 user 精准分流)
        let mut rules = Vec::new();

        // (1) 直连用户路由
        rules.push(json!({
            "user": ["user_direct"],
            "outbound": "out_direct"
        }));

        // (2) 每个 user_node_X 严格路由至对应的 out_socks_Y
        for i in 1..=total_users {
            let user_name = format!("user_node_{}", i);
            let out_port = OUTBOUND_START_PORT + (i as u16) - 1;
            rules.push(json!({
                "user": [user_name],
                "outbound": format!("out_socks_{}", out_port)
            }));
        }

        // 默认兜底
        rules.push(json!({
            "outbound": "out_direct"
        }));

        let config: Value = json!({
            "log": {
                "level": "warn",
                "timestamp": true
            },
            "inbounds": inbounds,
            "outbounds": outbounds,
            "route": {
                "rules": rules
            }
        });

        // 确保目标目录存在
        if let Some(parent) = Path::new(&self.config_path).parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut file = File::create(&self.config_path)?;
        file.write_all(serde_json::to_string_pretty(&config)?.as_bytes())?;

        info!("sing-box 双栈分流配置已成功生成并写入: {}", self.config_path);
        Ok(())
    }
}
