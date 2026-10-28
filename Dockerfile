# ================= 构建阶段 =================
# 使用包含最新稳定 Cargo 的 Rust 镜像 (避免依赖包的 edition2024 / new cargo 报错)
FROM rust:alpine3.21 AS builder

WORKDIR /usr/src/gatematrix
RUN apk add --no-cache musl-dev gcc make

# 拷贝依赖与锁文件以保证完全一致的依赖版本
COPY Cargo.toml Cargo.lock ./

# 拷贝真实源代码直接编译 (避免无锁文件触发升级到 edition2024 包)
COPY src ./src

# 构建最终的独立可执行二进制文件
RUN cargo build --release

# ================= 运行阶段 =================
FROM alpine:3.20

WORKDIR /app
# 安装基础证书、网络调试工具及 OpenVPN 拨号客户端
RUN apk add --no-cache ca-certificates iptables iproute2 curl tzdata openvpn

# 拷贝编译好的单二进制文件
COPY --from=builder /usr/src/gatematrix/target/release/gatematrix /app/gatematrix

EXPOSE 28080
ENTRYPOINT ["/app/gatematrix"]
