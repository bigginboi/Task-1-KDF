# Remote Build Utility — POC

Tauri 2 desktop application for remote compilation workflows.

## Prerequisites

- Node.js 18+
- Rust (via rustup)
- Docker

## Setup

### 1. Build the Docker image

```bash
docker build -t build-container docker/
```

### 2. Start the backend server

```bash
cd server
cargo run
```

Server runs on http://localhost:3001

### 3. Start the Tauri app

```bash
cd app
npm install
npm run tauri dev
```

## Usage

1. Click **Browse** to select a source folder
2. Click **Sync & Compile** to start the build workflow
3. View progress in the status area and output panel
4. Build artifacts are saved to `build-output/` inside the source folder

## API Endpoints

| Method | Path       | Description                    |
|--------|-----------|--------------------------------|
| POST   | `/sync`    | Upload project files           |
| PUT    | `/compile` | Trigger Docker-based compile   |
| GET    | `/output`  | Retrieve build artifacts/logs  |
