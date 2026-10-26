use crate::crawler::VpnNode;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::sync::Semaphore;
use tracing::{debug, info};

pub struct ProberEngine {
    max_concurrency: usize,
    timeout_duration: Duration,
}

impl ProberEngine {
    pub fn new(max_concurrency: usize, timeout_secs: u64) -> Self {
        Self {
            max_concurrency,
            timeout_duration: Duration::from_secs(timeout_secs),
        }
    }

    /// 并发批量探测节点列表健康度与延迟
    pub async fn probe_all(&self, nodes: &mut [VpnNode]) {
        let semaphore = Arc::new(Semaphore::new(self.max_concurrency));
        let mut tasks = Vec::with_capacity(nodes.len());

        info!("开始并发探活，待测节点数量: {}，并发限制: {}", nodes.len(), self.max_concurrency);

        for (idx, node) in nodes.iter().enumerate() {
            let permit = semaphore.clone();
            let ip = node.ip.clone();
            let ping_guess = node.ping_ms;
            let timeout = self.timeout_duration;

            let task = tokio::spawn(async move {
                let _p = permit.acquire_owned().await;
                let start = Instant::now();
                
                // VPNGate 节点通常开放 443、995 或 1194 等端口，此处先进行首选 TCP 探针
                // 尝试连接 443 端口，如果失败尝试其他常见端口
                let is_alive = match tokio::time::timeout(timeout, TcpStream::connect(format!("{}:443", ip))).await {
                    Ok(Ok(_)) => true,
                    _ => {
                        // 备用端口快速试探 995
                        matches!(
                            tokio::time::timeout(Duration::from_millis(1500), TcpStream::connect(format!("{}:995", ip))).await,
                            Ok(Ok(_))
                        )
                    }
                };

                let elapsed_ms = if is_alive {
                    start.elapsed().as_millis() as u64
                } else {
                    9999
                };

                (idx, is_alive, elapsed_ms, ping_guess)
            });

            tasks.push(task);
        }

        let mut alive_count = 0;
        for task in tasks {
            if let Ok((idx, is_alive, latency, ping_hint)) = task.await {
                let node = &mut nodes[idx];
                node.is_alive = is_alive;
                node.last_checked = Some(chrono::Utc::now());

                if is_alive {
                    alive_count += 1;
                    // 使用测量延迟与上游公布延迟结合
                    let measured = if latency < 9999 { latency } else { ping_hint };
                    node.latency_ms = Some(measured);
                    
                    // 计算综合评分: 延迟越低越好，速度越快越好
                    let speed_mbps = (node.speed_bps as f64) / 1_000_000.0;
                    let latency_score = (1000.0 / (measured as f64).max(10.0)) * 0.4;
                    let speed_score = speed_mbps.min(100.0) * 0.4;
                    let uptime_score = ((node.uptime_seconds as f64) / 86400.0).min(20.0);

                    node.dynamic_score = latency_score + speed_score + uptime_score;
                    debug!("节点 [{}] {}:{} 存活, RTT: {}ms, 评分: {:.2}", node.country_short, node.ip, 443, measured, node.dynamic_score);
                } else {
                    node.latency_ms = None;
                    node.dynamic_score = 0.0;
                }
            }
        }

        info!("探活完成，可用健康节点: {} / {}", alive_count, nodes.len());

        // 按动态评分从高到低排序，质量好的排在前面
        nodes.sort_by(|a, b| b.dynamic_score.partial_cmp(&a.dynamic_score).unwrap_or(std::cmp::Ordering::Equal));
    }
}
