# --- Stage 1: Build Frontend ---
FROM node:20-slim AS frontend-builder
WORKDIR /app/frontend
COPY frontend/package*.json ./
RUN npm install
COPY frontend/ ./
RUN npm run build

# --- Stage 2: Build Backend ---
FROM rust:1.77-bookworm AS backend-builder

# Install build dependencies for mediasoup (Python 3, make, g++)
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    python3 \
    python3-pip \
    make \
    g++ \
    && rustup component add rustfmt \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY Cargo.toml Cargo.lock ./

# Create a dummy source to cache dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm -rf src

# Now copy the real source and build
COPY src ./src
RUN touch src/main.rs && cargo build --release

# --- Stage 3: Runtime ---
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the binary from the builder
COPY --from=backend-builder /app/target/release/axum_rtc /app/axum_rtc

# Copy the frontend dist folder
COPY --from=frontend-builder /app/frontend/dist /app/frontend/dist

# Setup environment
ENV HOST=0.0.0.0
ENV PORT=3000
# ANNOUNCED_IP should be set in Dokploy environment variables

EXPOSE 3000
# Note: Mediasoup also requires a range of UDP ports (e.g., 10000-20000) 
# which should be configured in Dokploy/Docker port mapping.

CMD ["./axum_rtc"]
