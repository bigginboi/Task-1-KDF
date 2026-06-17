# Remote Build Utility

A desktop application and build backend that compiles local C, C++, and Rust source code inside a persistent Docker container.

## Architecture

- **Tauri App (`app/`)**: Desktop client (Vite, vanilla JS, and Rust) which walks directories, fetches base64 file arrays, fetches endpoints, and extracts zip outputs.
- **Actix Server (`server/`)**: Backend HTTP server managing rate limits, input safety validations, compilation timeouts, container execution, in-memory packaging, and automatic 1-hour workspace expiration.
- **Docker (`docker/`)**: Dockerfile definition specifying the persistent GCC and Rust build container environment.

## API Endpoints

| Method | Path | Content-Type | Description |
|--------|------|--------------|-------------|
| **POST** | `/api/sync?source_type=<type>` | `application/json` | Uploads base64 file entries to `./workspace/<uuid>/` and container's `/var/code/<uuid>`. |
| **PUT** | `/api/compile?workspace_id=<id>&source_type=<type>` | `application/json` | Runs compiler in container with `timeout 300s` and redirects to `/output/build.log`. |
| **GET** | `/api/output?workspace_id=<id>` | `application/octet-stream` | Returns ZIP stream of `/output/` folder containing compiled binaries and logs in-memory. |
| **DELETE** | `/api/workspace/{workspace_id}` | `application/json` | Manually destroys host and container workspace paths. |

## Quick Start

1. **Build and Run compiler container in daemon mode**:
   ```bash
   docker build -t build-container docker/
   docker run -d --name build-container --cpus 2 --memory 2g build-container sleep infinity
   ```

2. **Start Backend Actix Server**:
   ```bash
   cd server
   cargo run --release
   ```

3. **Start Tauri Desktop App**:
   ```bash
   cd app
   npm install
   npm run tauri dev
   ```

## Security & Cleanup Policy
- **Path Traversal Check**: Safe-character path matching whitelists file sync locations to prevent host injection.
- **Rate Limiting**: Exceeding 10 requests per minute per unique client IP results in `429 Too Many Requests`.
- **Autoclean Job**: Spawns a background thread running every 5 minutes that deletes local/container workspaces older than 1 hour.
