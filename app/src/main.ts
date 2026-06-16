// Tauri detection
const isTauri = typeof window !== 'undefined' && (window as any).__TAURI_INTERNALS__ !== undefined;

const folderPathInput = document.getElementById("folder-path") as HTMLInputElement;
const btnBrowse = document.getElementById("btn-browse") as HTMLButtonElement;
const btnCompile = document.getElementById("btn-compile") as HTMLButtonElement;
const statusSpan = document.getElementById("status") as HTMLSpanElement;
const statusDot = document.getElementById("status-dot") as HTMLSpanElement;
const outputArea = document.getElementById("output") as HTMLTextAreaElement;
const browserFolderInput = document.getElementById("browser-folder") as HTMLInputElement;
const btnClear = document.getElementById("btn-clear") as HTMLButtonElement;
const platformBadge = document.getElementById("platform-badge") as HTMLSpanElement;

let selectedFolder = "";
let selectedFiles: { path: string; content: string }[] = [];

// Platform label
if (platformBadge) {
  platformBadge.textContent = isTauri ? "tauri" : "browser";
}

// Tauri API lazy loading
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

const dotColors: Record<string, string> = {
  ready: "#d0d7de",
  working: "#dbab09",
  success: "#2da44e",
  error: "#cf222e",
};

function setStatus(text: string, level: "ready" | "working" | "success" | "error") {
  statusSpan.textContent = text;
  statusDot.style.background = dotColors[level] || "#d0d7de";
}

function appendLog(line: string) {
  outputArea.value += line + "\n";
  outputArea.scrollTop = outputArea.scrollHeight;
}

// Clear
btnClear?.addEventListener("click", () => {
  outputArea.value = "";
});

// Browse
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
          appendLog(`Selected: ${selectedFolder}`);
        }
      } catch (err: any) {
        setStatus("Error", "error");
        appendLog(`Error selecting folder: ${err}`);
      }
    }
  } else {
    browserFolderInput.click();
  }
});

// Browser folder picker
browserFolderInput.addEventListener("change", async (event: any) => {
  const filesList = event.target.files;
  if (filesList && filesList.length > 0) {
    selectedFiles = [];
    outputArea.value = "";
    setStatus("Reading...", "working");

    const firstFilePath = filesList[0].webkitRelativePath;
    const folderName = firstFilePath.split('/')[0] || "selected";
    selectedFolder = folderName;
    folderPathInput.value = folderName;

    appendLog("Reading files:");

    for (let i = 0; i < filesList.length; i++) {
      const file = filesList[i];
      const relPath = file.webkitRelativePath;
      const pathParts = relPath.split('/');
      pathParts.shift();
      const cleanPath = pathParts.join('/');
      if (!cleanPath) continue;

      try {
        const base64 = await readFileAsBase64(file);
        selectedFiles.push({ path: cleanPath, content: base64 });
        appendLog(`  ${cleanPath}`);
      } catch (e: any) {
        appendLog(`  [err] ${file.name}: ${e?.message || e}`);
      }
    }

    appendLog(`\n${selectedFiles.length} files loaded.`);
    btnCompile.disabled = false;
    setStatus("Ready", "ready");
  }
});

function readFileAsBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const result = reader.result as string;
      resolve(result.split(',')[1]);
    };
    reader.onerror = (e) => reject(e);
    reader.readAsDataURL(file);
  });
}

function downloadBase64File(filename: string, base64Content: string) {
  const bin = window.atob(base64Content);
  const bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) {
    bytes[i] = bin.charCodeAt(i);
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

// Compile
btnCompile.addEventListener("click", async () => {
  btnCompile.disabled = true;
  btnBrowse.disabled = true;

  if (isTauri) {
    if (!selectedFolder) return;
    outputArea.value = "";
    setStatus("Syncing...", "working");

    try {
      if (tauriInvoke) {
        const result = await tauriInvoke("sync_and_compile", {
          folderPath: selectedFolder,
        });

        if (result.success) {
          setStatus("Done", "success");
          appendLog("\n" + result.message);
        } else {
          setStatus("Failed", "error");
          appendLog("\n" + result.message);
        }
      }
    } catch (err: any) {
      setStatus("Error", "error");
      appendLog("\n" + (err?.toString() || "Unknown error"));
    } finally {
      btnCompile.disabled = false;
      btnBrowse.disabled = false;
    }
  } else {
    if (selectedFiles.length === 0) return;
    outputArea.value = "";
    setStatus("Syncing...", "working");
    appendLog("Syncing files to server...");

    try {
      const syncResp = await fetch("/sync", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ files: selectedFiles })
      });
      const syncResult = await syncResp.json();
      if (!syncResult.success) throw new Error(syncResult.message);
      appendLog(syncResult.message);

      setStatus("Compiling...", "working");
      appendLog("\nStarting compile container...");
      const compileResp = await fetch("/compile", { method: "PUT" });
      const compileResult = await compileResp.json();

      if (compileResult.logs) {
        for (const line of compileResult.logs) appendLog(line);
      }
      if (!compileResult.success) throw new Error("Compilation failed.");

      setStatus("Fetching output...", "working");
      appendLog("\nFetching build artifacts...");
      const outputResp = await fetch("/output");
      const outputResult = await outputResp.json();

      if (outputResult.logs) {
        for (const line of outputResult.logs) appendLog(line);
      }

      if (!outputResult.success) {
        if (outputResult.errors) {
          for (const err of outputResult.errors) appendLog(`error: ${err}`);
        }
        throw new Error("No output artifacts.");
      }

      if (outputResult.files) {
        appendLog("");
        for (const file of outputResult.files) {
          downloadBase64File(file.path, file.content);
          appendLog(`downloaded ${file.path}`);
        }
      }

      setStatus("Done", "success");
      appendLog("\nBuild complete.");
    } catch (err: any) {
      setStatus("Failed", "error");
      appendLog(`\nerror: ${err?.message || err?.toString() || "Unknown"}`);
    } finally {
      btnCompile.disabled = false;
      btnBrowse.disabled = false;
    }
  }
});
