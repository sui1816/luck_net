use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{error, info, warn};

/// VPNGate 节点结构体定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VpnNode {
    pub host_name: String,
    pub ip: String,
    pub score: u64,
    pub ping_ms: u64,
    pub speed_bps: u64,
    pub country_long: String,
    pub country_short: String,
    pub num_vpn_sessions: u64,
    pub uptime_seconds: u64,
    pub total_users: u64,
    pub total_traffic_bytes: u64,
    pub log_type: String,
    pub operator: String,
    pub message: String,
    #[serde(skip_serializing)]
    pub openvpn_config_base64: String,
    
    // 动态探测与健康状态字段
    #[serde(default)]
    pub is_alive: bool,
    #[serde(default)]
    pub latency_ms: Option<u64>,
    #[serde(default)]
    pub last_checked: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    pub dynamic_score: f64,
}

pub struct VpnGateCrawler {
    api_url: String,
    client: reqwest::Client,
}

impl VpnGateCrawler {
    pub fn new(api_url: Option<String>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(20))
            .gzip(true)
            .build()
            .unwrap_or_default();

        Self {
            api_url: api_url.unwrap_or_else(|| "https://www.vpngate.net/api/iphone/".to_string()),
            client,
        }
    }

    /// 从 VPNGate 获取原始 CSV 并解析为结构化节点列表
    pub async fn fetch_nodes(&self) -> Result<Vec<VpnNode>, anyhow::Error> {
        info!("正在从 VPNGate 同步节点列表: {}", self.api_url);
        
        let response = match self.client.get(&self.api_url).send().await {
            Ok(resp) => resp,
            Err(e) => {
                error!("VPNGate API 请求失败: {:?}", e);
                return Err(anyhow::anyhow!("VPNGate API 请求失败: {}", e));
            }
        };

        let text = response.text().await?;
        let nodes = Self::parse_csv(&text)?;
        info!("VPNGate 同步完成，成功解析出 {} 个可用候选节点", nodes.len());
        Ok(nodes)
    }

    /// 解析 VPNGate 官方返回的特殊 CSV 格式
    /// 头部通常带有注释和 '*vpn_servers' 标志
    pub fn parse_csv(csv_content: &str) -> Result<Vec<VpnNode>, anyhow::Error> {
        let mut nodes = Vec::new();
        let lines: Vec<&str> = csv_content.lines().collect();

        // 查找有效 CSV header 所在行
        let mut header_idx = None;
        for (i, line) in lines.iter().enumerate() {
            if line.starts_with("#HostName") || line.starts_with("*vpn_servers") {
                header_idx = Some(i);
                break;
            }
        }

        let start_idx = match header_idx {
            Some(idx) => idx,
            None => {
                warn!("未能识别 VPNGate CSV 格式头，尝试按标准头解析");
                0
            }
        };

        // 重新拼装符合 csv crate 规范的纯文本
        let valid_csv_data = lines[start_idx..].join("\n");
        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(true)
            .flexible(true)
            .from_reader(valid_csv_data.as_bytes());

        for result in rdr.records() {
            let record = match result {
                Ok(rec) => rec,
                Err(_) => continue,
            };

            // 过滤末尾声明行（如 *）
            if record.len() < 15 || record.get(0).unwrap_or("").starts_with('*') {
                continue;
            }

            let host_name = record.get(0).unwrap_or("").to_string();
            let ip = record.get(1).unwrap_or("").to_string();
            let score = record.get(2).and_then(|v| v.parse().ok()).unwrap_or(0);
            let ping_ms = record.get(3).and_then(|v| v.parse().ok()).unwrap_or(999);
            let speed_bps = record.get(4).and_then(|v| v.parse().ok()).unwrap_or(0);
            let country_long = record.get(5).unwrap_or("Unknown").to_string();
            let country_short = record.get(6).unwrap_or("--").to_string();
            let num_vpn_sessions = record.get(7).and_then(|v| v.parse().ok()).unwrap_or(0);
            let uptime_seconds = record.get(8).and_then(|v| v.parse().ok()).unwrap_or(0);
            let total_users = record.get(9).and_then(|v| v.parse().ok()).unwrap_or(0);
            let total_traffic_bytes = record.get(10).and_then(|v| v.parse().ok()).unwrap_or(0);
            let log_type = record.get(11).unwrap_or("").to_string();
            let operator = record.get(12).unwrap_or("").to_string();
            let message = record.get(13).unwrap_or("").to_string();
            let openvpn_config_base64 = record.get(14).unwrap_or("").to_string();

            if ip.is_empty() || openvpn_config_base64.is_empty() {
                continue;
            }

            nodes.push(VpnNode {
                host_name,
                ip,
                score,
                ping_ms,
                speed_bps,
                country_long,
                country_short,
                num_vpn_sessions,
                uptime_seconds,
                total_users,
                total_traffic_bytes,
                log_type,
                operator,
                message,
                openvpn_config_base64,
                is_alive: true,
                latency_ms: None,
                last_checked: None,
                dynamic_score: score as f64,
            });
        }

        Ok(nodes)
    }
}
