#!/usr/bin/env bash
# ==============================================================================
# GateMatrix 聚合一键安装与自愈引导脚本
# 支持单行运行：bash <(curl -sL https://raw.githubusercontent.com/sui1816/luck_net/main/install.sh)
# ==============================================================================

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;36m'
PLAIN='\033[0m'

echo -e "${BLUE}================================================================${PLAIN}"
echo -e "${BLUE}   GateMatrix - VPNGate 多出口落地代理矩阵聚合安装程序           ${PLAIN}"
echo -e "${BLUE}================================================================${PLAIN}"

# 1. 检查 root 权限
if [ "$(id -u)" != "0" ]; then
    echo -e "${RED}错误: 必须以 root 权限运行此脚本。请使用: sudo bash $0${PLAIN}"
    exit 1
fi

# 2. 自动安装基础系统组件 (git, curl)
echo -e "${YELLOW}正在检测并准备基础运行环境...${PLAIN}"
if command -v apt-get >/dev/null 2>&1; then
    apt-get update -q 2>&1 >/dev/null || true
    apt-get install -y -q git curl tar openssl >/dev/null 2>&1 || true
elif command -v yum >/dev/null 2>&1; then
    yum install -y git curl tar openssl >/dev/null 2>&1 || true
elif command -v dnf >/dev/null 2>&1; then
    dnf install -y git curl tar openssl >/dev/null 2>&1 || true
elif command -v apk >/dev/null 2>&1; then
    apk add --no-cache git curl tar openssl bash >/dev/null 2>&1 || true
fi

# 3. 克隆或拉取最新项目代码至 /opt/luck_net
TARGET_DIR="/opt/luck_net"
REPO_URL="https://github.com/sui1816/luck_net.git"

if [ -d "${TARGET_DIR}/.git" ]; then
    echo -e "${GREEN}检测到已有项目目录，正在同步拉取最新代码...${PLAIN}"
    cd "${TARGET_DIR}"
    git pull origin main || git pull origin master || true
else
    echo -e "${GREEN}正在从 GitHub 克隆项目代码到 ${TARGET_DIR}...${PLAIN}"
    mkdir -p "${TARGET_DIR}"
    git clone "${REPO_URL}" "${TARGET_DIR}"
    cd "${TARGET_DIR}"
fi

# 4. 赋予执行权限并无缝移交至 deploy.sh
chmod +x deploy.sh
echo -e "${GREEN}项目准备就绪，正在启动交互式配置...${PLAIN}\n"
bash deploy.sh
