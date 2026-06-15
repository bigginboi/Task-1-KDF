import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";

const folderPathInput = document.getElementById("folder-path") as HTMLInputElement;
const btnBrowse = document.getElementById("btn-browse") as HTMLButtonElement;
const btnCompile = document.getElementById("btn-compile") as HTMLButtonElement;
const statusDiv = document.getElementById("status") as HTMLDivElement;
const outputArea = document.getElementById("output") as HTMLTextAreaElement;

let selectedFolder = "";

function setStatus(text: string, level: "ready" | "working" | "success" | "error") {
  statusDiv.textContent = text;
  statusDiv.className = `status-${level}`;
}

function appendLog(line: string) {
  outputArea.value += line + "\n";
  outputArea.scrollTop = outputArea.scrollHeight;
}

btnBrowse.addEventListener("click", async () => {
  const folder = await open({ directory: true, multiple: false });
  if (folder) {
    selectedFolder = folder as string;
    folderPathInput.value = selectedFolder;
    btnCompile.disabled = false;
    outputArea.value = "";
    setStatus("Ready", "ready");
  }
});

listen<string>("build-log", (event) => {
  appendLog(event.payload);
});

listen<string>("build-status", (event) => {
  setStatus(event.payload, "working");
});

btnCompile.addEventListener("click", async () => {
  if (!selectedFolder) return;

  btnCompile.disabled = true;
  btnBrowse.disabled = true;
  outputArea.value = "";
  setStatus("Syncing Files...", "working");

  try {
    const result = await invoke<{ success: boolean; message: string }>("sync_and_compile", {
      folderPath: selectedFolder,
    });

    if (result.success) {
      setStatus("Complete", "success");
      appendLog("\n" + result.message);
    } else {
      setStatus("Failed", "error");
      appendLog("\nBuild failed: " + result.message);
    }
  } catch (err: any) {
    setStatus("Failed", "error");
    appendLog("\nError: " + (err?.toString() || "Unknown error"));
  } finally {
    btnCompile.disabled = false;
    btnBrowse.disabled = false;
  }
});
