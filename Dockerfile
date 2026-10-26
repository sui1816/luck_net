# ================= 构建阶段 =================
FROM rust:1.80-alpine AS builder

WORKDIR /usr/src/gatematrix
RUN apk add --no-cache musl-dev gcc make

# 拷贝依赖配置
COPY Cargo.toml ./

# 占位预热构建以缓存 crates
RUN mkdir src && echo "fn main() {}" > src/main.rs && cargo build --release && rm -rf src

# 拷贝真实源代码
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
