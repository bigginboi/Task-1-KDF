const { invoke } = window.__TAURI__.core;
const { open } = window.__TAURI__.dialog;

const getApiBase = () => {
    if (typeof window !== 'undefined' && window.__API_URL__) {
        return window.__API_URL__;
    }
    return 'http://127.0.0.1:3001/api';
};
const API = getApiBase();

let selectedPath = null;
let sourceType = null;

const browseBtn = document.getElementById('browseBtn');
const compileBtn = document.getElementById('compileBtn');
const pathDisplay = document.getElementById('selectedPath');
const typeDisplay = document.getElementById('sourceType');
const fileListEl = document.getElementById('fileList');
const fileSection = document.getElementById('fileSection');
const logOutput = document.getElementById('logOutput');
const logSection = document.getElementById('logSection');
const statusMsg = document.getElementById('statusMessage');
const statusSection = document.getElementById('statusSection');

browseBtn.addEventListener('click', browse);
compileBtn.addEventListener('click', compile);

async function browse() {
    const folder = await open({ directory: true, title: 'Select project folder' });
    if (!folder) return;

    selectedPath = folder;
    pathDisplay.textContent = folder;

    const entries = await invoke('read_dir', { path: folder });
    const names = entries.map(e => e.name);

    sourceType = detectType(names);

    if (!sourceType) {
        typeDisplay.textContent = 'No C/C++/Rust source detected';
        typeDisplay.className = 'type-unknown';
        compileBtn.disabled = true;
        return;
    }

    typeDisplay.textContent = 'Detected: ' + sourceType.toUpperCase();
    typeDisplay.className = 'type-detected';
    showFiles(entries);
    compileBtn.disabled = false;
}

function detectType(filenames) {
    if (filenames.some(f => f === 'Cargo.toml')) return 'rust';
    if (filenames.some(f => /\.(cpp|cc|cxx|hpp)$/i.test(f))) return 'cpp';
    if (filenames.some(f => /\.c$/i.test(f))) return 'c';
    return null;
}

function showFiles(entries) {
    fileListEl.innerHTML = '';
    entries.filter(e => !e.is_dir).forEach(f => {
        const div = document.createElement('div');
        div.textContent = f.name;
        fileListEl.appendChild(div);
    });
    fileSection.style.display = 'block';
}

async function compile() {
    browseBtn.disabled = true;
    compileBtn.disabled = true;
    logSection.style.display = 'block';
    statusSection.style.display = 'block';
    logOutput.textContent = '';

    try {
        setStatus('Scanning folder...', 'info');
        log('Reading project files...');
        const files = await invoke('get_project_files', { path: selectedPath });
        log('Found ' + files.length + ' file(s)');

        setStatus('Syncing files...', 'info');
        log('Uploading files to server...');

        const syncResp = await fetch(API + '/sync?source_type=' + sourceType, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ files }),
        });
        const syncData = await syncResp.json();

        if (!syncResp.ok) {
            throw new Error(syncData.message || syncData.error_code || 'Upload failed');
        }

        const workspaceId = syncData.workspace_id;
        log('Workspace synchronized. ID: ' + workspaceId);

        setStatus('Compiling (' + sourceType.toUpperCase() + ')...', 'info');
        log('\nStarting compilation...\n');

        const compileResp = await fetch(
            API + '/compile?workspace_id=' + workspaceId + '&source_type=' + sourceType,
            { method: 'PUT' }
        );
        const compileData = await compileResp.json();

        if (!compileResp.ok) {
            throw new Error(compileData.message || compileData.error_code || 'Compilation execution failed');
        }

        if (compileData.output) {
            log(compileData.output);
        }

        setStatus('Retrieving files...', 'info');
        log('\nRetrieving build output...');

        try {
            const outputResp = await fetch(API + '/output?workspace_id=' + workspaceId);

            if (outputResp.ok) {
                const ct = outputResp.headers.get('content-type') || '';

                if (ct.includes('octet-stream')) {
                    const buffer = await outputResp.arrayBuffer();
                    const outputB64 = bytesToBase64(new Uint8Array(buffer));
                    const filesExtracted = await invoke('extract_zip', {
                        data: outputB64,
                        destPath: selectedPath
                    });
                    log('Saved build artifacts to build/ (' + filesExtracted.length + ' files)');
                } else {
                    const errBody = await outputResp.json();
                    log('No output files: ' + (errBody.message || errBody.error_code || 'unknown'));
                }
            } else {
                try {
                    const errBody = await outputResp.json();
                    log('No output files: ' + (errBody.message || errBody.error_code || outputResp.statusText));
                } catch (_) {
                    log('No output files available');
                }
            }
        } catch (getErr) {
            log('Warning: ' + getErr.message);
        }

        if (compileData.success) {
            setStatus('Build complete', 'success');
            log('\nBuild completed successfully');
        } else {
            setStatus('Build failed', 'error');
            log('\nCompilation failed (exit ' + compileData.status_code + ')');
        }

    } catch (err) {
        setStatus('Error: ' + err.message, 'error');
        log('\nError: ' + err.message);
    } finally {
        browseBtn.disabled = false;
        compileBtn.disabled = false;
    }
}

function setStatus(msg, type) {
    statusMsg.textContent = msg;
    statusMsg.className = 'status-' + type;
    statusSection.style.display = 'block';
}

function log(text) {
    logOutput.textContent += text + '\n';
    logOutput.scrollTop = logOutput.scrollHeight;
}

function bytesToBase64(bytes) {
    let bin = '';
    for (let i = 0; i < bytes.length; i += 8192) {
        const chunk = bytes.subarray(i, Math.min(i + 8192, bytes.length));
        bin += String.fromCharCode.apply(null, chunk);
    }
    return btoa(bin);
}
