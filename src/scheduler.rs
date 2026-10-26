use crate::crawler::VpnNode;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;

pub const INBOUND_START_PORT: u16 = 50001;
pub const INBOUND_END_PORT: u16 = 55000;
pub const OUTBOUND_START_PORT: u16 = 55001;
pub const OUTBOUND_END_PORT: u16 = 60000;
pub const DIRECT_PORT: u16 = 60001;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortMapping {
    pub inbound_port: u16,
    pub outbound_socks_port: u16,
    pub target_node: Option<VpnNode>,
    pub is_direct: bool,
    pub description: String,
}

pub struct MatrixScheduler {
    pub mappings: Arc<DashMap<u16, PortMapping>>,
}

impl MatrixScheduler {
    pub fn new() -> Self {
        let mappings = Arc::new(DashMap::new());

        // 初始化直连端口 60001
        mappings.insert(
            DIRECT_PORT,
            PortMapping {
                inbound_port: DIRECT_PORT,
                outbound_socks_port: 0,
                target_node: None,
                is_direct: true,
                description: "VPS 原生直连出口 (不走中继)".to_string(),
            },
        );

        // 初始化 5000 个备用端口映射结构
        for offset in 0..=(INBOUND_END_PORT - INBOUND_START_PORT) {
            let in_port = INBOUND_START_PORT + offset;
            let out_port = OUTBOUND_START_PORT + offset;

            mappings.insert(
                in_port,
                PortMapping {
                    inbound_port: in_port,
                    outbound_socks_port: out_port,
                    target_node: None,
                    is_direct: false,
                    description: format!("出口通道 #{}", offset + 1),
                },
            );
        }

        Self { mappings }
    }

    /// 应用经过评分与探活的最新节点列表到 5000 个端口上
    pub fn update_nodes(&self, active_nodes: &[VpnNode]) {
        if active_nodes.is_empty() {
            info!("无可用存活节点，跳过端口映射刷新");
            return;
        }

        let total_active = active_nodes.len();
        info!("正在将 {} 个高评分节点映射至 5000 个端口中...", total_active);

        // 遍历所有待分配的 5000 个端口
        for offset in 0..=(INBOUND_END_PORT - INBOUND_START_PORT) {
            let in_port = INBOUND_START_PORT + offset;
            let out_port = OUTBOUND_START_PORT + offset;

            // 动态映射策略:
            // 前 N 个端口严格 1-to-1 分配最好的前 N 个节点
            // 后续端口若无足够节点，则按哈希环循环负载复用，确保 5000 个端口 100% 连通可用
            let node_index = if (offset as usize) < total_active {
                offset as usize
            } else {
                (offset as usize) % total_active
            };

            let node = &active_nodes[node_index];

            let tag = if (offset as usize) < 500 {
                format!("Top-500 黄金出口 ({}·{}ms)", node.country_short, node.latency_ms.unwrap_or(0))
            } else {
                format!("长效出口 ({}·{})", node.country_short, node.ip)
            };

            self.mappings.insert(
                in_port,
                PortMapping {
                    inbound_port: in_port,
                    outbound_socks_port: out_port,
                    target_node: Some(node.clone()),
                    is_direct: false,
                    description: tag,
                },
            );
        }

        info!("5000 端口映射更新完成，全部通道就绪");
    }

    /// 获取当前所有的端口映射快照（支持按分页或数量获取）
    pub fn get_mappings_snapshot(&self, limit: usize) -> Vec<PortMapping> {
        let mut list: Vec<PortMapping> = self.mappings.iter().map(|item| item.value().clone()).collect();
        list.sort_by_key(|m| m.inbound_port);
        if limit > 0 && limit < list.len() {
            list.truncate(limit);
        }
        list
    }
}
