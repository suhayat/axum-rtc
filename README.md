# Axum RTC

Axum RTC is an experimental project built with **Rust**, using **Axum** as the web framework and **mediasoup** as the WebRTC SFU (Selective Forwarding Unit).  
This project is intended for learning and exploration purposes in building real-time communication systems.

---

## 🚀 Features

- Built with Rust and Axum
- Uses mediasoup as SFU
- WebRTC-based real-time communication
- Dockerized for easy setup

---

## 📦 Installation

### 1. Using Dockerfile

You can build and run the project using the provided Dockerfile.

#### Build the image
```bash
docker build -t axum-rtc .
```

#### Run the container
```bash
docker run -d \
  -p 3000:3000 \
  -p 10000-10100:10000-10100/udp \
  --env-file .env \
  axum-rtc
```

---

### 2. Using Docker Compose

Alternatively, you can use Docker Compose for easier orchestration.

#### Example `docker-compose.yml`
```yaml
version: "3.8"

services:
  axum-rtc:
    build: .
    container_name: axum-rtc
    ports:
      - "3000:3000"
      - "10000-10100:10000-10100/udp"
    env_file:
      - .env
    restart: unless-stopped
```

#### Run the service
```bash
docker-compose up -d --build
```

---

## 🌐 Networking

This project requires UDP ports for WebRTC communication.

Make sure to open the following port range:

10000-10100/udp

If you're deploying on a cloud server (AWS, GCP, etc.), ensure the firewall/security group allows this range.

---

## ⚙️ Environment Variables

Create a `.env` file in the root directory:

```env
HOST=0.0.0.0
PORT=3000
ANNOUNCED_IP=your_public_ip
```

### Description

- **HOST**  
  The address where the server will bind.

- **PORT**  
  The HTTP server port.

- **ANNOUNCED_IP**  
  Public IP address used by mediasoup for WebRTC communication.  
  This is important if your server is behind NAT or running in the cloud.

---

## 🧪 Notes

- This is a hobby/experimental project, not production-ready.
- mediasoup requires proper networking configuration, especially in NAT environments.
- Make sure your server supports UDP traffic.

---

## 📄 License

MIT (or specify your license here)

---

## 🤝 Contributing

Feel free to fork and experiment. Contributions are welcome, but keep in mind this project is mainly for learning purposes.
