# Process Specification: Remote Build Utility API

This document specifies the communication protocol, endpoint definitions, security guidelines, and workspace lifecycle management for the Remote Build Utility.

---

## 1. Architectural Workflow

```
[ Tauri App Client ]                     [ Actix-Web Backend ]                     [ Docker Daemon ]
       |                                           |                                       |
       |--- 1. POST /api/sync -------------------->|                                       |
       |    (JSON base64 files)                    |--- 2. Write locally ----------------->|
       |                                           |       (./workspace/<uuid>/)           |
       |                                           |--- 3. Copy files to container ------->|
       |                                           |       (docker cp to /var/code/)       |
       |<-- 4. Sync Response (workspace_id) -------|                                       |
       |                                           |                                       |
       |--- 5. PUT /api/compile ------------------>|                                       |
       |    (workspace_id & source_type)           |--- 6. docker exec compile ----------->|
       |                                           |       (standard output/main)          |
       |                                           |       (build.log & exit status)       |
       |<-- 7. Compile Response (logs/exit code) --|                                       |
       |                                           |                                       |
       |--- 8. GET /api/output ------------------->|                                       |
       |    (workspace_id)                         |--- 9. docker exec zip --------------->|
       |                                           |       (pipe raw output zip stream)    |
       |<-- 10. File Stream (ZIP bytes) -----------|                                       |
```

---

## 2. API Endpoints

### 2.1 POST `/api/sync`
Synchronizes the workspace files by accepting a JSON payload containing base64-encoded file contents.

- **Query Parameters**:
  - `source_type`: `c` | `cpp` | `rust` (Required)
- **Request Headers**:
  - `Content-Type`: `application/json`
- **Request Body JSON Schema**:
  ```json
  {
    "type": "object",
    "required": ["files"],
    "properties": {
      "files": {
        "type": "array",
        "items": {
          "type": "object",
          "required": ["path", "content"],
          "properties": {
            "path": { "type": "string" },
            "content": { "type": "string" }
          }
        }
      }
    }
  }
  ```
- **Response Schema (200 OK)**:
  ```json
  {
    "success": true,
    "workspace_id": "uuid-v4-string",
    "source_type": "cpp"
  }
  ```

---

### 2.2 PUT `/api/compile`
Triggers the Docker compilation workflow inside the persistent container.

- **Query Parameters**:
  - `workspace_id`: The workspace UUID returned from the sync endpoint (Required)
  - `source_type`: `c` | `cpp` | `rust` (Required)
- **Compilation Execution**:
  - Command executes inside container `build-container` using `docker exec -w /var/code/<workspace_id>`.
  - Max compile timeout is `300s`.
  - Output binary target location: `output/main`.
  - Compilation output redirected to `output/build.log` with compiler exit code appended.
- **Response Schema (200 OK)**:
  ```json
  {
    "success": true,
    "workspace_id": "uuid-v4-string",
    "status_code": 0,
    "output": "Compiling..."
  }
  ```

---

### 2.3 GET `/api/output`
Downloads compilation artifacts as an in-memory ZIP archive stream.

- **Query Parameters**:
  - `workspace_id`: The workspace UUID (Required)
- **Response Headers**:
  - `Content-Type`: `application/octet-stream`
  - `Content-Disposition`: `attachment; filename="<workspace_id>-output.zip"`
- **Response Body**:
  - Raw ZIP bytes containing the compiled binary (`main` or `main.exe` equivalent) and the compilation log file (`build.log`).

---

### 2.4 DELETE `/api/workspace/{workspace_id}`
Manually requests deletion of workspace storage on both the host and the container.

- **Response Schema (200 OK)**:
  ```json
  {
    "success": true,
    "workspace_id": "uuid-v4-string",
    "message": "Workspace deleted"
  }
  ```

---

## 3. Error Handling Specification

All error responses return structured JSON schemas with standard machine-readable codes.

### 3.1 Error Response Schema
```json
{
  "success": false,
  "error_code": "ERROR_CODE_STRING",
  "message": "A human readable explanation of the failure"
}
```

### 3.2 Error Code Mapping

| Error Code | HTTP Status | Description |
|------------|-------------|-------------|
| `INVALID_SOURCE_TYPE` | 400 | The `source_type` query parameter is missing or invalid. |
| `INVALID_PATH` | 400 | The file path contains traversal elements (`..`), absolute paths, or invalid characters. |
| `PAYLOAD_TOO_LARGE` | 507 | The total payload exceeds 100MB or an individual file exceeds 50MB. |
| `DOCKER_UNAVAILABLE` | 503 | Docker daemon is not running or the container is unreachable. |
| `WORKSPACE_NOT_FOUND` | 404 | The requested workspace UUID folder does not exist or has expired. |
| `FILE_WRITE_FAILED` | 500 | Failed to write the uploaded file contents to the host disk. |
| `DOCKER_EXEC_FAILED` | 500 | Docker execution, copying, or compilation encountered a system error or timeout. |
| `RATE_LIMIT_EXCEEDED` | 429 | The client IP has exceeded the allowed limit of 10 requests per minute. |

---

## 4. Rate Limiting Policy
- **Limits**: 10 requests per minute per unique client IP address.
- **Scope**: Applied across all `/api/*` endpoints.
- **Handling**: Exceeded rates immediately return `429 Too Many Requests` with the `RATE_LIMIT_EXCEEDED` code.

---

## 5. Security & Validation

### 5.1 File Path Whitelist
To prevent directory traversal and host system injection:
- File paths must **not** start with a slash (`/`, `\\`) or contain parent directory segments (`..`).
- Path characters are whitelisted to: `[a-zA-Z0-9_.-/\\\\]`.

### 5.2 Network and Sandboxing (Recommended)
- Docker container runs with `--network none` during compilation to prevent network access from compiled binaries.
- Resource limits are constrained to `--cpus 2` and `--memory 2g` to prevent host CPU/Memory exhaustion.

---

## 6. Migration Guide for Existing Clients

```diff
- // OLD: Synchronizing via Multipart Form ZIP file
- const form = new FormData();
- form.append('file', zipBlob);
- fetch('/api/sync?source_type=cpp', { method: 'POST', body: form });

+ // NEW: Synchronizing via JSON payload of base64-encoded files
+ const files = [
+   { path: "main.cpp", content: "I2luY2x1ZGUgPGlvc3RyZWFtPi..." }
+ ];
+ fetch('/api/sync?source_type=cpp', {
+   method: 'POST',
+   headers: { 'Content-Type': 'application/json' },
+   body: JSON.stringify({ files })
+ });
```
