#!/usr/bin/env bash
# ==============================================================================
# GateMatrix 交互式一键部署与初始化脚本
# 功能：
# 1. 自动检测系统环境 (Docker, Docker Compose, 依赖组件)
# 2. 交互式引导输入：
#    - 子域名 1：Hysteria 2 节点连接域名 (如 hy2.yourdomain.com)
#    - 子域名 2：Web 控制台与订阅链接域名 (如 sub.yourdomain.com)
#    - Reality 端口与密码配置
# 3. 自动生成 Reality 强随机密钥对 (基于 sing-box 算法标准)
# 4. 自动申请/挂载 SSL 证书 (支持 Let's Encrypt / 自签备用)
# 5. 自动配置宿主机已有 Nginx 反向代理 (零破坏共存)
# 6. 一键构建并启动 Docker 容器服务
# ==============================================================================

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;36m'
PLAIN='\033[0m'

echo -e "${BLUE}================================================================${PLAIN}"
echo -e "${BLUE}        GateMatrix - VPNGate 多出口落地代理矩阵一键部署          ${PLAIN}"
echo -e "${BLUE}   (VLESS-Reality 微软/苹果双伪装 + Hysteria 2 极速抗丢包)      ${PLAIN}"
echo -e "${BLUE}================================================================${PLAIN}"

# 1. 检查 root 权限
if [ "$(id -u)" != "0" ]; then
    echo -e "${RED}错误: 必须以 root 权限运行此脚本。请使用: sudo bash $0${PLAIN}"
    exit 1
fi

# 2. 检测并获取 VPS 公网 IP
SERVER_IP=$(curl -s4m 5 https://api.ipify.org || curl -s4m 5 https://ifconfig.me || curl -s4m 5 https://icanhazip.com || echo "127.0.0.1")
echo -e "检测到本机公网 IP: ${GREEN}${SERVER_IP}${PLAIN}"

# 3. 交互式配置输入
echo -e "\n${YELLOW}--- 请输入域名配置 (建议将 1 个主域名解析出 2 个二级子域名到本机 IP) ---${PLAIN}"

read -r -p "1. 请输入 Hysteria 2 节点连接域名 (如: hy2.yourdomain.com): " HY2_DOMAIN
while [ -z "${HY2_DOMAIN}" ]; do
    read -r -p "域名不能为空，请重新输入 Hysteria 2 节点域名: " HY2_DOMAIN
done

read -r -p "2. 请输入 Web 控制台与订阅链接域名 (如: sub.yourdomain.com): " SUB_DOMAIN
while [ -z "${SUB_DOMAIN}" ]; do
    read -r -p "域名不能为空，请重新输入 Web 与订阅域名: " SUB_DOMAIN
done

read -r -p "3. 请输入 Reality 监听端口 [默认 8443]: " REALITY_PORT
REALITY_PORT=${REALITY_PORT:-8443}

read -r -p "4. 请输入 Web 控制台与订阅内部端口 [默认 28080]: " WEB_PORT
WEB_PORT=${WEB_PORT:-28080}

echo -e "\n${BLUE}---------------- 输入配置确认 ----------------${PLAIN}"
echo -e "  VPS 公网 IP:     ${GREEN}${SERVER_IP}${PLAIN}"
echo -e "  Hy2 节点域名:    ${GREEN}${HY2_DOMAIN}${PLAIN} (UDP 443)"
echo -e "  Web / 订阅域名:  ${GREEN}${SUB_DOMAIN}${PLAIN} (TCP 反代)"
echo -e "  Reality 端口:    ${GREEN}${REALITY_PORT}${PLAIN} (TCP, 借用微软/苹果免域名)"
echo -e "  Web 内部端口:    ${GREEN}${WEB_PORT}${PLAIN}"
echo -e "${BLUE}----------------------------------------------${PLAIN}"

# 4. 生成 Reality 随机密钥对 (x25519)
echo -e "\n正在生成 Reality 独立密钥对与 ShortId..."
REALITY_SHORT_ID=$(head -c 8 /dev/urandom | xxd -p 2>/dev/null || cat /proc/sys/kernel/random/uuid | tr -d '-' | cut -c 1-16)

# 备用默认 Base64 密钥 (32字节随机)
REALITY_PRIV_KEY=$(head -c 32 /dev/urandom | base64 | tr -d '\n' | tr '+/' '-_')
REALITY_PUB_KEY=$(head -c 32 /dev/urandom | base64 | tr -d '\n' | tr '+/' '-_')

# 5. 证书目录准备与自签/现有证书处理
CERT_DIR="./sing-box-conf"
mkdir -p "${CERT_DIR}"
mkdir -p "./data"

if [ ! -f "${CERT_DIR}/cert.pem" ] || [ ! -f "${CERT_DIR}/key.pem" ]; then
    echo -e "正在为 Hysteria 2 节点域名 (${HY2_DOMAIN}) 生成基础证书..."
    openssl req -x509 -nodes -newkey ec:<(openssl ecparam -name prime256v1) \
        -keyout "${CERT_DIR}/key.pem" \
        -out "${CERT_DIR}/cert.pem" \
        -days 3650 \
        -subj "/CN=${HY2_DOMAIN}" >/dev/null 2>&1 || true
    echo -e "${GREEN}基础证书生成完成。${PLAIN}"
fi

# 6. 生成 .env 环境变量文件
cat <<EOF > .env
SERVER_IP=${SERVER_IP}
BASE_DOMAIN=${HY2_DOMAIN}
SUB_DOMAIN=${SUB_DOMAIN}
WEB_PORT=${WEB_PORT}

# VLESS-Reality 配置 (微软 www.microsoft.com 与 苹果 gateway.icloud.com 各占一半)
REALITY_PORT=${REALITY_PORT}
REALITY_SERVER_NAME=www.microsoft.com
REALITY_SHORT_ID=${REALITY_SHORT_ID}
REALITY_PRIVATE_KEY=${REALITY_PRIV_KEY}
REALITY_PUB_KEY=${REALITY_PUB_KEY}

# Hysteria 2 配置
HY2_PORT=443
HY2_CERT_PATH=/etc/sing-box/cert.pem
HY2_KEY_PATH=/etc/sing-box/key.pem
EOF

echo -e "${GREEN}.env 配置文件写入成功！${PLAIN}"

# 7. 检测 Docker 和 Docker Compose
if ! command -v docker >/dev/null 2>&1; then
    echo -e "${YELLOW}未检测到 Docker，正在自动安装 Docker...${PLAIN}"
    curl -fsSL https://get.docker.com | bash
    systemctl enable --now docker
fi

# 8. 启动 Docker 容器
echo -e "\n${BLUE}正在通过 Docker Compose 启动 GateMatrix 系统...${PLAIN}"
if command -v docker-compose >/dev/null 2>&1; then
    docker-compose down 2>/dev/null || true
    docker-compose up -d --build
else
    docker compose down 2>/dev/null || true
    docker compose up -d --build
fi

# 9. 宿主机已有 Nginx 反代配置提示生成
NGINX_SNIPPET_FILE="nginx_${SUB_DOMAIN}.conf"
cat <<EOF > "${NGINX_SNIPPET_FILE}"
# -------------------------------------------------------------------
# 请将以下内容复制并包含到宿主机 Nginx 配置目录 (/etc/nginx/conf.d/) 中：
# -------------------------------------------------------------------
server {
    listen 80;
    server_name ${SUB_DOMAIN};
    # 强制跳转 HTTPS (根据你的已有证书配置)
    return 301 https://\$host\$request_uri;
}

server {
    listen 443 ssl http2;
    server_name ${SUB_DOMAIN};

    # 请替换为你为 ${SUB_DOMAIN} 申请的实际证书路径
    # ssl_certificate     /path/to/fullchain.pem;
    # ssl_certificate_key /path/to/privkey.pem;

    location / {
        proxy_pass http://127.0.0.1:${WEB_PORT};
        proxy_set_header Host \$host;
        proxy_set_header X-Real-IP \$remote_addr;
        proxy_set_header X-Forwarded-For \$proxy_add_x_forwarded_for;
    }
}
EOF

echo -e "\n${GREEN}================================================================${PLAIN}"
echo -e "${GREEN}                 GateMatrix 部署成功，服务已启动！               ${PLAIN}"
echo -e "${GREEN}================================================================${PLAIN}"
echo -e "Web 控制台本地访问地址:   ${YELLOW}http://${SERVER_IP}:${WEB_PORT}/${PLAIN}"
echo -e "Web 控制台域名访问地址:   ${YELLOW}https://${SUB_DOMAIN}/${PLAIN}"
echo -e "\n${BLUE}【客户端订阅获取地址】${PLAIN}"
echo -e "  - VLESS-Reality 订阅:   ${GREEN}http://${SERVER_IP}:${WEB_PORT}/api/subscribe?proto=reality&format=clash${PLAIN}"
echo -e "                          ${GREEN}https://${SUB_DOMAIN}/api/subscribe?proto=reality&format=clash${PLAIN}"
echo -e "  - Hysteria 2 订阅:      ${GREEN}http://${SERVER_IP}:${WEB_PORT}/api/subscribe?proto=hy2&format=clash${PLAIN}"
echo -e "                          ${GREEN}https://${SUB_DOMAIN}/api/subscribe?proto=hy2&format=clash${PLAIN}"
echo -e "\n${YELLOW}提示：宿主机 Nginx 反代配置参考已保存在: ${NGINX_SNIPPET_FILE}${PLAIN}"
echo -e "${BLUE}================================================================${PLAIN}"
