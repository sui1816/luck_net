mod crawler;
mod prober;
mod scheduler;
mod singbox;
mod tunnel;
mod web;

use crawler::VpnGateCrawler;
use prober::ProberEngine;
use scheduler::MatrixScheduler;
use singbox::SingBoxConfigManager;
use std::env;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info};
use tunnel::TunnelManager;
use web::AppState;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // 初始化日志记录
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,gatematrix=debug".into()),
        )
        .init();

    info!("============================================================");
    info!("   GateMatrix - VLESS-Reality & Hysteria2 双栈矩阵启动中   ");
    info!("============================================================");

    // 读取环境变量配置
    let domain = env::var("BASE_DOMAIN").unwrap_or_else(|_| "vpn.yourdomain.com".to_string());
    let server_ip = env::var("SERVER_IP").unwrap_or_else(|_| "192.129.241.48".to_string());
    let web_port: u16 = env::var("WEB_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(28080);

    let reality_port: u16 = env::var("REALITY_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8443);
    let hy2_port: u16 = env::var("HY2_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(443);

    // Reality 配置 (免域名，借用微软)
    let reality_private_key = env::var("REALITY_PRIVATE_KEY")
        .unwrap_or_else(|_| "6N_Q5_O3j3K1_M4-L1X1Z2_A3-B4_C5-D6_E7-F8_G0=".to_string());
    let reality_pub_key = env::var("REALITY_PUB_KEY")
        .unwrap_or_else(|_| "1A_B2_C3_D4-E5_F6-G7_H8-I9_J0-K1_L2-M3_N4_O5=".to_string());
    let reality_short_id = env::var("REALITY_SHORT_ID").unwrap_or_else(|_| "0123456789abcdef".to_string());
    let reality_server_name = env::var("REALITY_SERVER_NAME").unwrap_or_else(|_| "www.microsoft.com".to_string());

    // Hy2 证书配置
    let max_hot_tunnels: usize = env::var("MAX_HOT_TUNNELS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(35); // 默认 35 条最优住宅隧道常驻，内存 < 250MB，CPU < 1%
    let cert_path = env::var("HY2_CERT_PATH").unwrap_or_else(|_| "/etc/sing-box/cert.pem".to_string());
    let key_path = env::var("HY2_KEY_PATH").unwrap_or_else(|_| "/etc/sing-box/key.pem".to_string());

    let singbox_conf_path = env::var("SINGBOX_CONF_PATH")
        .unwrap_or_else(|_| "/etc/sing-box/config.json".to_string());

    // 初始化核心调度器与管理器
    let scheduler = Arc::new(MatrixScheduler::new());
    let config_manager = Arc::new(SingBoxConfigManager::new(
        &singbox_conf_path,
        &domain,
        reality_port,
        hy2_port,
        &reality_private_key,
        &reality_short_id,
        &reality_server_name,
        &cert_path,
        &key_path,
    ));

    // 生成初始化 sing-box 双栈配置文件
    if let Err(e) = config_manager.generate_and_save(100) {
        error!("初始化 sing-box 双栈配置失败: {:?}", e);
    }

    // 启动后台定时任务：爬取、探活与动态端口调度及隧道落地挂载
    let crawler = Arc::new(VpnGateCrawler::new(None));
    let prober = Arc::new(ProberEngine::new(100, 3));
    let tunnel_manager = Arc::new(TunnelManager::new("/app/data/tunnels"));
    let sched_clone = scheduler.clone();
    let conf_clone = config_manager.clone();
    let tunnel_clone = tunnel_manager.clone();

    tokio::spawn(async move {
        // 初始延迟 1 秒后开始第一次同步
        tokio::time::sleep(Duration::from_secs(1)).await;

        loop {
            info!("--> 启动新一轮 VPNGate 节点爬取与健康检测流程...");
            match crawler.fetch_nodes().await {
                Ok(mut nodes) => {
                    info!("获取到原始节点 {} 个，开始探活评分...", nodes.len());
                    prober.probe_all(&mut nodes).await;

                    // 过滤出真正存活的健康节点
                    let active_nodes: Vec<_> = nodes.into_iter().filter(|n| n.is_alive).collect();
                    let active_len = active_nodes.len();
                    info!("存活健康节点数量: {}", active_len);

                    if active_len > 0 {
                        // 1. 动态更新出口映射与质量排序
                        sched_clone.update_nodes(&active_nodes);

                        // 2. 动态打通底层 VPN 加密隧道，挂载至 55001+ 本地端口池 (热池控制)
                        tunnel_clone.sync_and_launch_tunnels(&active_nodes, max_hot_tunnels).await;

                        // 3. 动态重新生成 sing-box 配置文件
                        if let Err(e) = conf_clone.generate_and_save(active_len) {
                            error!("刷新 sing-box 配置失败: {:?}", e);
                        }
                    }
                }
                Err(e) => {
                    error!("VPNGate 同步出错: {:?}", e);
                }
            }

            // 探活与更新周期: 15 分钟
            info!("本轮调度完成，将在 15 分钟后进行下一次全网节点探活与重排。");
            tokio::time::sleep(Duration::from_secs(15 * 60)).await;
        }
    });

    // 启动 Web 控制台与订阅 API 服务
    let app_state = AppState {
        scheduler: scheduler.clone(),
        domain,
        server_ip,
        reality_port,
        hy2_port,
        reality_pub_key,
        reality_short_id,
        reality_server_name,
    };

    let app = web::create_router(app_state);
    let addr = SocketAddr::from(([0, 0, 0, 0], web_port));
    info!("Web 控制台与客户端订阅接口已监听在: http://0.0.0.0:{}", web_port);
    info!("Reality 订阅: http://0.0.0.0:{}/api/subscribe?proto=reality&format=clash", web_port);
    info!("Hysteria2 订阅: http://0.0.0.0:{}/api/subscribe?proto=hy2&format=clash", web_port);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
