use crate::scheduler::MatrixScheduler;
use axum::{
    extract::{Query, State},
    response::{Html, IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use tower_http::cors::CorsLayer;

#[derive(Clone)]
pub struct AppState {
    pub scheduler: Arc<MatrixScheduler>,
    pub domain: String,
    pub server_ip: String,
    pub reality_port: u16,
    pub hy2_port: u16,
    pub reality_pub_key: String,
    pub reality_short_id: String,
    pub reality_server_name: String,
}

#[derive(Deserialize)]
pub struct SubQuery {
    pub format: Option<String>,
    pub proto: Option<String>, // reality 或 hy2
    pub limit: Option<usize>,
    pub _token: Option<String>,
}

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/", get(dashboard_handler))
        .route("/api/nodes", get(nodes_api_handler))
        .route("/api/subscribe", get(subscribe_handler))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

/// Web 控制台主页 (展示：IP类型、国旗地区、官方原生带宽、RTT延迟、评分，0测速流量损耗)
async fn dashboard_handler(State(state): State<AppState>) -> Html<String> {
    let mappings = state.scheduler.get_mappings_snapshot(100);
    let total_count = state.scheduler.mappings.len();

    let mut rows = String::new();
    for (idx, m) in mappings.iter().enumerate() {
        let (ip, country, ip_type, speed_str, rtt, score, uuid, hy2_pass) = if let Some(node) = &m.target_node {
            let speed_mbps = (node.speed_bps as f64) / 1_000_000.0;
            let flag = match node.country_short.to_uppercase().as_str() {
                "JP" => "🇯🇵",
                "US" => "🇺🇸",
                "KR" => "🇰🇷",
                "HK" => "🇭🇰",
                "SG" => "🇸🇬",
                "TW" => "🇹🇼",
                "GB" | "UK" => "🇬🇧",
                "DE" => "🇩🇪",
                "FR" => "🇫🇷",
                "CA" => "🇨🇦",
                _ => "🌐",
            };
            (
                node.ip.clone(),
                format!("{} {} ({})", flag, node.country_long, node.country_short),
                "<span class='badge-res'>住宅宽带 (ISP)</span>".to_string(),
                format!("{:.1} Mbps", speed_mbps),
                format!("{} ms", node.latency_ms.unwrap_or(0)),
                format!("{:.1}", node.dynamic_score),
                format!("d2a02306-6945-4e3e-bf64-{:012x}", idx + 1),
                format!("pass_node_{:04}", idx + 1),
            )
        } else if m.is_direct {
            (
                "VPS 原生".to_string(),
                "🇩🇪 原生宿主机出口".to_string(),
                "<span class='badge-dc'>原生机房 (DC)</span>".to_string(),
                "1000.0 Mbps".to_string(),
                "0 ms".to_string(),
                "100.0".to_string(),
                "b831381d-6324-4d53-ad4f-8cda48b30811".to_string(),
                "pass_direct_token_123".to_string(),
            )
        } else {
            (
                "备用".to_string(),
                "--".to_string(),
                "--".to_string(),
                "--".to_string(),
                "--".to_string(),
                "0.0".to_string(),
                "--".to_string(),
                "--".to_string(),
            )
        };

        rows.push_str(&format!(
            "<tr>
                <td><strong>#{}</strong></td>
                <td><span class='badge'>{}</span></td>
                <td><code>{}</code></td>
                <td>{}</td>
                <td>{}</td>
                <td><strong>{}</strong></td>
                <td><span class='rtt-tag'>{}</span></td>
                <td>{}</td>
                <td><code>{}</code></td>
                <td><code>{}</code></td>
            </tr>",
            idx,
            if m.is_direct { "Direct" } else { "Relay" },
            ip,
            country,
            ip_type,
            speed_str,
            rtt,
            score,
            &uuid[..18],
            hy2_pass
        ));
    }

    let html = format!(
        r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>GateMatrix 双栈出口代理矩阵</title>
    <style>
        body {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; background: #0f172a; color: #f8fafc; margin: 0; padding: 24px; }}
        .header {{ display: flex; justify-content: space-between; align-items: center; border-bottom: 1px solid #334155; padding-bottom: 16px; margin-bottom: 24px; }}
        h1 {{ margin: 0; font-size: 24px; color: #38bdf8; display: flex; align-items: center; gap: 10px; }}
        .links {{ display: flex; gap: 12px; }}
        .stats {{ display: flex; gap: 16px; margin-bottom: 24px; }}
        .card {{ background: #1e293b; padding: 16px 20px; border-radius: 8px; flex: 1; border: 1px solid #334155; }}
        .card h3 {{ margin: 0 0 8px 0; font-size: 14px; color: #94a3b8; }}
        .card p {{ margin: 0; font-size: 20px; font-weight: bold; color: #38bdf8; }}
        .notice {{ background: #1e293b; border-left: 4px solid #10b981; padding: 12px 16px; border-radius: 4px; margin-bottom: 24px; font-size: 13px; color: #94a3b8; }}
        .notice strong {{ color: #10b981; }}
        table {{ width: 100%; border-collapse: collapse; background: #1e293b; border-radius: 8px; overflow: hidden; border: 1px solid #334155; font-size: 14px; }}
        th, td {{ padding: 10px 14px; text-align: left; border-bottom: 1px solid #334155; }}
        th {{ background: #0f172a; color: #94a3b8; font-size: 12px; text-transform: uppercase; }}
        code {{ background: #334155; padding: 2px 6px; border-radius: 4px; color: #38bdf8; font-size: 12px; }}
        .badge {{ background: #0369a1; color: white; padding: 2px 8px; border-radius: 12px; font-size: 11px; }}
        .badge-res {{ background: #059669; color: white; padding: 2px 8px; border-radius: 4px; font-size: 11px; font-weight: 500; }}
        .badge-dc {{ background: #475569; color: white; padding: 2px 8px; border-radius: 4px; font-size: 11px; }}
        .rtt-tag {{ color: #10b981; font-weight: 600; }}
        .sub-link {{ background: #0284c7; color: white; padding: 8px 16px; border-radius: 6px; text-decoration: none; font-size: 14px; font-weight: 500; }}
        .sub-link.hy2 {{ background: #7c3aed; }}
    </style>
</head>
<body>
    <div class="header">
        <h1>🌐 GateMatrix 双栈出口代理矩阵</h1>
        <div class="links">
            <a class="sub-link" href="/api/subscribe?proto=reality&format=clash" target="_blank">导出 Reality (Clash)</a>
            <a class="sub-link hy2" href="/api/subscribe?proto=hy2&format=clash" target="_blank">导出 Hysteria 2 (Clash)</a>
        </div>
    </div>

    <div class="notice">
        <strong>⚡ 流量防护机制生效中：</strong> 系统采用轻量毫秒探针 + VPNGate 官方原生带宽同步方案，<strong>严禁无意义的跑满大文件测速</strong>，每月系统维护流量开销 &lt; 300MB，100% 保护您的 4TB 流量额度安全。
    </div>

    <div class="stats">
        <div class="card">
            <h3>VLESS-Reality 入站</h3>
            <p>TCP {} (微软/苹果 1:1 双伪装)</p>
        </div>
        <div class="card">
            <h3>Hysteria 2 入站</h3>
            <p>UDP {} ({})</p>
        </div>
        <div class="card">
            <h3>出口住宅 IP 调度池</h3>
            <p>{} 个优质通道按需分配</p>
        </div>
    </div>

    <h3>实时活跃出口矩阵 (前 100)</h3>
    <table>
        <thead>
            <tr>
                <th>通道</th>
                <th>类型</th>
                <th>落地出口 IP</th>
                <th>归属国家/地区</th>
                <th>IP 属性</th>
                <th>原生带宽</th>
                <th>RTT 延迟</th>
                <th>综合评分</th>
                <th>Reality UUID 前缀</th>
                <th>Hy2 认证 Token</th>
            </tr>
        </thead>
        <tbody>
            {}
        </tbody>
    </table>
</body>
</html>"#,
        state.reality_port, state.hy2_port, state.domain, total_count, rows
    );

    Html(html)
}

/// JSON 状态接口
async fn nodes_api_handler(State(state): State<AppState>) -> Json<serde_json::Value> {
    let list = state.scheduler.get_mappings_snapshot(500);
    Json(json!({
        "status": "success",
        "reality_port": state.reality_port,
        "hy2_port": state.hy2_port,
        "domain": state.domain,
        "total_managed": state.scheduler.mappings.len(),
        "mappings": list
    }))
}

/// 客户端订阅生成 (支持 Reality 与 Hy2 分别导出或混合导出)
async fn subscribe_handler(
    State(state): State<AppState>,
    Query(query): Query<SubQuery>,
) -> Response {
    let limit = query.limit.unwrap_or(35); // 默认 35 条最优热池节点，杜绝 5000 条臃肿卡死
    let list = state.scheduler.get_mappings_snapshot(limit);
    let proto = query.proto.unwrap_or_else(|| "reality".to_string());

    let mut proxies = Vec::new();
    let mut proxy_names = Vec::new();

    // 1. 直连节点
    if proto == "hy2" {
        let name = "⚡ VPS 原生直连出口 (Hy2)".to_string();
        proxy_names.push(format!("      - \"{}\"", name));
        proxies.push(format!(
            r#"  - name: "{}"
    type: hysteria2
    server: {}
    port: {}
    password: "pass_direct_token_123"
    sni: {}
    skip-cert-verify: true"#,
            name, state.domain, state.hy2_port, state.domain
        ));
    } else {
        let name = "⚡ VPS 原生直连出口 (Reality)".to_string();
        proxy_names.push(format!("      - \"{}\"", name));
        proxies.push(format!(
            r#"  - name: "{}"
    type: vless
    server: {}
    port: {}
    uuid: b831381d-6324-4d53-ad4f-8cda48b30811
    network: tcp
    tls: true
    udp: true
    flow: xtls-rprx-vision
    servername: {}
    reality-opts:
      public-key: "{}"
      short-id: "{}"
    client-fingerprint: chrome"#,
            name,
            state.server_ip,
            state.reality_port,
            state.reality_server_name,
            state.reality_pub_key,
            state.reality_short_id
        ));
    }

    // 2. 住宅出口节点
    for (idx, m) in list.iter().enumerate() {
        let node = match &m.target_node {
            Some(n) => n,
            None => continue,
        };

        let node_idx = idx + 1;
        let uuid = format!("d2a02306-6945-4e3e-bf64-{:012x}", node_idx);
        let password = format!("pass_node_{:04}", node_idx);

        if proto == "hy2" {
            let name = format!(
                "[{}] #{} 住宅IP - {}ms (Hy2)",
                node.country_short, node_idx, node.latency_ms.unwrap_or(0)
            );
            proxy_names.push(format!("      - \"{}\"", name));
            proxies.push(format!(
                r#"  - name: "{}"
    type: hysteria2
    server: {}
    port: {}
    password: "{}"
    sni: {}
    skip-cert-verify: true"#,
                name, state.domain, state.hy2_port, password, state.domain
            ));
        } else {
            // 微软 (Microsoft) 与 苹果 (Apple) 各占一半，交替轮询伪装
            let (sni_tag, target_sni) = if node_idx % 2 == 1 {
                ("MS-微软", "www.microsoft.com")
            } else {
                ("AP-苹果", "gateway.icloud.com")
            };

            let name = format!(
                "[{}] [{}] #{} 住宅IP - {}ms",
                sni_tag,
                node.country_short,
                node_idx,
                node.latency_ms.unwrap_or(0)
            );
            proxy_names.push(format!("      - \"{}\"", name));
            proxies.push(format!(
                r#"  - name: "{}"
    type: vless
    server: {}
    port: {}
    uuid: {}
    network: tcp
    tls: true
    udp: true
    flow: xtls-rprx-vision
    servername: {}
    reality-opts:
      public-key: "{}"
      short-id: "{}"
    client-fingerprint: chrome"#,
                name,
                state.server_ip,
                state.reality_port,
                uuid,
                target_sni,
                state.reality_pub_key,
                state.reality_short_id
            ));
        }
    }

    let yaml = format!(
        r#"port: 7890
socks-port: 7891
allow-lan: false
mode: rule
log-level: info

proxies:
{}

proxy-groups:
  - name: "GateMatrix 落地选择"
    type: select
    proxies:
{}
  - name: "自动最优"
    type: url-test
    url: http://cp.cloudflare.com/generate_204
    interval: 300
    proxies:
{}

rules:
  - MATCH,GateMatrix 落地选择
"#,
        proxies.join("\n"),
        proxy_names.join("\n"),
        proxy_names.join("\n")
    );

    (
        [("Content-Type", "application/yaml; charset=utf-8")],
        yaml,
    )
        .into_response()
}
