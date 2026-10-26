# GateMatrix - VLESS-Reality & Hysteria2 双栈出口代理矩阵系统

基于 **Rust** 异步高并发控制引擎与 **sing-box** 官方核心构建的动态住宅出口落地代理矩阵。

针对已有生产业务（如已运行 Nginx/Docker）的 VPS 进行深度优化，**彻底摒弃公网裸开数千个端口的落后做法**，采用现代最先进的 **单端口多用户分流架构** 与 **SOCKS5 本地解耦转接桥**。

---

## 🌟 核心双栈与分流架构

1. **VLESS-Reality 极隐蔽入站 (高位端口 TCP 8443)**：
   * **完全免域名、免申请证书**；
   * **微软（`www.microsoft.com`）与苹果（`gateway.icloud.com`）各占 50% 伪装**，节点交替轮换，模拟混合设备真实流量，抗审查能力顶格。
2. **Hysteria 2 极速抗丢包入站 (UDP 443)**：
   * 绑定你的真实域名，跑在 **UDP 443** 端口；
   * **与宿主机已有的 TCP 443 Nginx 网站 100% 完美共存、零冲突**（Linux 内核 TCP/UDP 端口天然隔离）。
3. **SOCKS5 内部解耦转接桥 (127.0.0.1:55001 ~ 55000+N)**：
   * 充当 sing-box 与底层各个 VPN 拨号网卡之间“即插即用”的接力棒；
   * 绝不修改宿主机全局默认网关（`route-nopull`），宿主机原有的 SSH、Docker、网站不受任何干扰。
4. **防卡死与低功耗控制 (热池 30~50 个常驻)**：
   * 常驻 35 个全球最优住宅节点，总内存仅占用 **约 250MB，CPU < 1%**；
   * 80ms 错峰拉起（杜绝 CPU 尖刺） + 30 秒断流自愈超时（杜绝僵尸进程）。

---

## 🚀 极简交互式部署 (推荐)

只需将本项目上传至 VPS 任意目录（如 `/opt/luck_net`），运行交互脚本：

```bash
cd /opt/luck_net
chmod +x deploy.sh
bash deploy.sh
```

脚本会交互式引导输入：
1. **Hysteria 2 节点域名**（如 `hy2.yourdomain.com`，需提前解析 A 记录到 VPS）；
2. **Web 与订阅域名**（如 `sub.yourdomain.com`，用于查看控制台与导出订阅）；
3. **Reality 监听端口**（默认 `8443`）。

脚本将自动完成：
- 随机生成 Reality x25519 密钥对与 ShortId；
- 为 Hy2 生成预备证书；
- 生成宿主机已有 Nginx 的二级域名反代配置参考；
- 一键构建并启动 Docker 容器。

---

## 🌐 访问与使用

* **Web 管理控制台**：`https://sub.yourdomain.com/` (或 `http://VPS_IP:28080/`)
* **导出 VLESS-Reality 订阅 (Clash)**：
  `https://sub.yourdomain.com/api/subscribe?proto=reality&format=clash`
* **导出 Hysteria 2 订阅 (Clash)**：
  `https://sub.yourdomain.com/api/subscribe?proto=hy2&format=clash`

---

## ⚙️ 核心参数调优 (`.env`)

```ini
SERVER_IP=192.129.241.48
BASE_DOMAIN=hy2.yourdomain.com
WEB_PORT=28080

# Reality 端口与密钥
REALITY_PORT=8443
REALITY_SERVER_NAME=www.microsoft.com
REALITY_SHORT_ID=0123456789abcdef
REALITY_PRIVATE_KEY=...
REALITY_PUB_KEY=...

# Hysteria 2 端口
HY2_PORT=443

# 常驻住宅隧道并发数 (推荐 30~50)
MAX_HOT_TUNNELS=35
```
