# Remote Build Utility — Setup Guide

Follow these steps to configure, build, and run the remote compilation tool.

## Prerequisites

Ensure you have the following installed on your machine:
- **Node.js** 18+ (with npm)
- **Rust** (stable compiler and cargo toolchain)
- **Docker** (Docker Desktop must be running)

---

## 1. Build the Docker Image

Build the compilation container image from the `docker/` directory:

```bash
docker build -t build-container docker/
```

This sets up a standard GCC build environment with our auto-detection compile script.

---

## 2. Compile and Start the Server

1. Navigate to the `server/` directory:
   ```bash
   cd server
   ```
2. Build and run the Actix HTTP backend:
   ```bash
   cargo run --release
   ```

The server starts listening on `http://127.0.0.1:3001`.

---

## 3. Build and Start the Tauri App

1. Navigate to the `app/` directory:
   ```bash
   cd app
   ```
2. Install dependencies:
   ```bash
   npm install
   ```
3. Run the development environment:
   ```bash
   npm run tauri dev
   ```

Or create a production build of the desktop application:
```bash
npm run tauri build
```

---

## 4. How to Use

1. Click **Browse Folder** and select any local directory containing your C/C++ source code (must contain either a `Makefile` or `.cpp`/`.c` files).
2. Click **Compile**.
3. View real-time output in the logs panel. On successful build, the compiled binaries will be saved under the local project's `build/` directory.
