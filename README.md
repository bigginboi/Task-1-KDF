# remote-build-utility

A local tool that syncs source files to a backend server, compiles them inside a Docker container, and returns the build artifacts. Works as both a Tauri desktop app and a browser-based UI.

## What it does

1. You pick a folder containing C/C++ source files.
2. The files get uploaded to a local Actix Web server.
3. The server mounts them into a `gcc` Docker container and runs the build.
4. The compiled output is sent back — either saved to disk (Tauri) or downloaded via the browser.

## Project structure

```
.
├── app/                    # Tauri v2 frontend (TypeScript + Rust)
│   ├── src/                # Frontend source (main.ts, styles.css)
│   ├── src-tauri/          # Tauri Rust backend (lib.rs, main.rs)
│   └── package.json
├── server/                 # Actix Web backend
│   └── src/main.rs         # API server + static file server
├── docker/
│   └── Dockerfile          # gcc container with auto-detect build script
└── README.md
```

## Requirements

- [Node.js](https://nodejs.org/) 18+
- [Rust](https://rustup.rs/) (stable toolchain)
- [Docker](https://www.docker.com/products/docker-desktop/)

## Getting started

### 1. Build the Docker image

```bash
docker build -t build-container docker/
```

This pulls `gcc:latest` and sets up a build script that auto-detects Makefiles or compiles `.cpp` files directly.

### 2. Start the server

```bash
cd server
cargo run
```

Runs on `http://localhost:3001`. This serves both the API endpoints and the web UI (from `app/dist/`).

### 3a. Use it in the browser

Open `http://localhost:3001` in any browser. Click Browse to select a folder, then click Run build.

Make sure the frontend is built first:

```bash
cd app
npm install
npm run build
```

### 3b. Use it as a desktop app (Tauri)

```bash
cd app
npm install
npm run tauri dev
```

Or for a release build:

```bash
cd app/src-tauri
cargo run --release
```

## API

The server exposes three endpoints. The browser UI and the Tauri app both use these under the hood.

### `POST /sync`

Upload files to the server workspace. Expects JSON:

```json
{
  "files": [
    {
      "path": "main.cpp",
      "content": "<base64-encoded file content>"
    }
  ]
}
```

Returns `{ "success": true, "message": "3 files synced" }`.

### `PUT /compile`

Runs `docker run --rm -v <workspace>:/src build-container`. The container auto-detects whether to use `make` or compile `.cpp` files directly.

Returns `{ "success": true, "logs": ["Build completed: build/output"] }`.

### `GET /output`

Returns base64-encoded build artifacts from the workspace `build/` directory.

```json
{
  "success": true,
  "files": [{ "path": "output", "content": "<base64>" }],
  "logs": ["1 artifact(s) ready"]
}
```

## How the Docker build works

The Dockerfile uses `gcc:latest` and runs a shell script that checks, in order:

1. If a `Makefile` exists → runs `make`
2. If any `.cpp` files exist → compiles them with `g++ -o build/output`
3. Otherwise → exits with error

Build output goes to `/src/build/` inside the container, which is the mounted workspace directory.

## Notes

- The server binds to `127.0.0.1:3001` (localhost only). It does not listen on external interfaces.
- In the browser, files are read client-side using the File API and base64-encoded before upload. There is no streaming.
- The Tauri app uses native file dialogs via `tauri-plugin-dialog` and reads files from disk directly.
- On Windows, the server strips the `\\?\` prefix from canonicalized paths before passing them to Docker.
