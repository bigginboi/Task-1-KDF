// Detection for Tauri vs standard browser
const isTauri = typeof window !== 'undefined' && (window as any).__TAURI_INTERNALS__ !== undefined;

const folderPathInput = document.getElementById("folder-path") as HTMLInputElement;
const btnBrowse = document.getElementById("btn-browse") as HTMLButtonElement;
const btnCompile = document.getElementById("btn-compile") as HTMLButtonElement;
const statusDiv = document.getElementById("status") as HTMLDivElement;
const outputArea = document.getElementById("output") as HTMLTextAreaElement;
const browserFolderInput = document.getElementById("browser-folder") as HTMLInputElement;
const btnClear = document.getElementById("btn-clear") as HTMLButtonElement;
const platformBadge = document.getElementById("platform-badge") as HTMLSpanElement;

let selectedFolder = "";
let selectedFiles: { path: string; content: string }[] = [];

// Initialize platform UI badge
if (platformBadge) {
  if (isTauri) {
    platformBadge.textContent = "Tauri Desktop";
    platformBadge.className = "platform-badge platform-tauri";
  } else {
    platformBadge.textContent = "Web Browser";
    platformBadge.className = "platform-badge platform-browser";
  }
}

// Dynamically load Tauri APIs only if we are running inside Tauri
let tauriInvoke: any = null;
let tauriListen: any = null;
let tauriOpen: any = null;

if (isTauri) {
  Promise.all([
    import("@tauri-apps/api/core"),
    import("@tauri-apps/api/event"),
    import("@tauri-apps/plugin-dialog")
  ]).then(([{ invoke }, { listen }, { open }]) => {
    tauriInvoke = invoke;
    tauriListen = listen;
    tauriOpen = open;

    // Listen to real-time build events from Rust backend
    tauriListen("build-log", (event: any) => {
      appendLog(event.payload);
    });

    tauriListen("build-status", (event: any) => {
      setStatus(event.payload, "working");
    });
  }).catch((err) => {
    console.error("Failed to load Tauri plugins:", err);
  });
}

function setStatus(text: string, level: "ready" | "working" | "success" | "error") {
  statusDiv.textContent = text;
  statusDiv.className = `status-badge status-${level}`;
}

function appendLog(line: string) {
  outputArea.value += line + "\n";
  outputArea.scrollTop = outputArea.scrollHeight;
}

// Clear logs button
if (btnClear) {
  btnClear.addEventListener("click", () => {
    outputArea.value = "";
  });
}

// Folder Selection trigger
btnBrowse.addEventListener("click", async () => {
  if (isTauri) {
    if (tauriOpen) {
      try {
        const folder = await tauriOpen({ directory: true, multiple: false });
        if (folder) {
          selectedFolder = folder as string;
          folderPathInput.value = selectedFolder;
          btnCompile.disabled = false;
          outputArea.value = "";
          setStatus("Ready", "ready");
          appendLog(`Selected workspace: ${selectedFolder}`);
        }
      } catch (err: any) {
        setStatus("Failed", "error");
        appendLog(`Error selecting folder: ${err}`);
      }
    }
  } else {
    // In browser, trigger hidden folder input click
    browserFolderInput.click();
  }
});

// Browser-specific folder input change event
browserFolderInput.addEventListener("change", async (event: any) => {
  const filesList = event.target.files;
  if (filesList && filesList.length > 0) {
    selectedFiles = [];
    outputArea.value = "";
    setStatus("Reading workspace...", "working");

    // Extract folder name from the first file path
    const firstFilePath = filesList[0].webkitRelativePath;
    const folderName = firstFilePath.split('/')[0] || "Selected Folder";
    selectedFolder = folderName;
    folderPathInput.value = folderName;

    appendLog(`Reading workspace files:`);

    for (let i = 0; i < filesList.length; i++) {
      const file = filesList[i];
      const relPath = file.webkitRelativePath;
      
      // Strip top-level folder name (matches Tauri behavior)
      const pathParts = relPath.split('/');
      pathParts.shift(); 
      const cleanPath = pathParts.join('/');

      if (!cleanPath) continue;

      try {
        const base64 = await readFileAsBase64(file);
        selectedFiles.push({ path: cleanPath, content: base64 });
        appendLog(`  + ${cleanPath}`);
      } catch (e: any) {
        appendLog(`  [ERROR] Failed to read ${file.name}: ${e?.message || e}`);
      }
    }

    appendLog(`\nSuccessfully loaded ${selectedFiles.length} files. Ready to sync and compile.`);
    btnCompile.disabled = false;
    setStatus("Ready", "ready");
  }
});

// Helper to read file as Base64 in browser
function readFileAsBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const result = reader.result as string;
      const base64 = result.split(',')[1];
      resolve(base64);
    };
    reader.onerror = (e) => reject(e);
    reader.readAsDataURL(file);
  });
}

// Helper to trigger browser downloads for compiled files
function downloadBase64File(filename: string, base64Content: string) {
  const binaryString = window.atob(base64Content);
  const len = binaryString.length;
  const bytes = new Uint8Array(len);
  for (let i = 0; i < len; i++) {
    bytes[i] = binaryString.charCodeAt(i);
  }
  const blob = new Blob([bytes], { type: "application/octet-stream" });
  const url = URL.createObjectURL(blob);
  
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}

// Sync & Compile Action handler
btnCompile.addEventListener("click", async () => {
  btnCompile.disabled = true;
  btnBrowse.disabled = true;
  
  if (isTauri) {
    if (!selectedFolder) return;
    outputArea.value = "";
    setStatus("Syncing Files...", "working");

    try {
      if (tauriInvoke) {
        const result = await tauriInvoke("sync_and_compile", {
          folderPath: selectedFolder,
        });

        if (result.success) {
          setStatus("Complete", "success");
          appendLog("\n" + result.message);
        } else {
          setStatus("Failed", "error");
          appendLog("\nBuild failed: " + result.message);
        }
      }
    } catch (err: any) {
      setStatus("Failed", "error");
      appendLog("\nError: " + (err?.toString() || "Unknown error"));
    } finally {
      btnCompile.disabled = false;
      btnBrowse.disabled = false;
    }
  } else {
    // Standard Browser workflow
    if (selectedFiles.length === 0) return;
    outputArea.value = "";
    setStatus("Syncing Files...", "working");
    appendLog("Syncing files to backend server...");

    try {
      // Step 1: POST /sync
      const syncResp = await fetch("/sync", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ files: selectedFiles })
      });
      const syncResult = await syncResp.json();
      if (!syncResult.success) {
        throw new Error(syncResult.message);
      }
      appendLog(`Server response: ${syncResult.message}`);

      // Step 2: PUT /compile
      setStatus("Compiling...", "working");
      appendLog("\nRequesting Docker build container...");
      const compileResp = await fetch("/compile", { method: "PUT" });
      const compileResult = await compileResp.json();

      if (compileResult.logs) {
        for (const logLine of compileResult.logs) {
          appendLog(logLine);
        }
      }

      if (!compileResult.success) {
        throw new Error("Compilation container exited with error.");
      }

      // Step 3: GET /output
      setStatus("Retrieving Output...", "working");
      appendLog("\nRetrieving build outputs from server...");
      const outputResp = await fetch("/output");
      const outputResult = await outputResp.json();

      if (outputResult.logs) {
        for (const logLine of outputResult.logs) {
          appendLog(logLine);
        }
      }

      if (!outputResult.success) {
        if (outputResult.errors) {
          for (const err of outputResult.errors) {
            appendLog(`ERROR: ${err}`);
          }
        }
        throw new Error("No output artifacts found.");
      }

      // Download all returned files in browser
      if (outputResult.files) {
        appendLog("");
        for (const file of outputResult.files) {
          downloadBase64File(file.path, file.content);
          appendLog(`[DOWNLOAD] Saved ${file.path} to downloads folder.`);
        }
      }

      setStatus("Complete", "success");
      appendLog("\nBuild completed successfully!");
    } catch (err: any) {
      setStatus("Failed", "error");
      appendLog(`\n[ERROR] ${err?.message || err?.toString() || "Unknown error"}`);
    } finally {
      btnCompile.disabled = false;
      btnBrowse.disabled = false;
    }
  }
});
