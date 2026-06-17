use std::process::Command;

const CONTAINER: &str = "build-server";
const IMAGE: &str = "build-container";

pub fn ensure_running() -> Result<(), String> {
    let output = Command::new("docker")
        .args(["inspect", "-f", "{{.State.Running}}", CONTAINER])
        .output()
        .map_err(|e| format!("docker not available: {}", e))?;

    let running = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if running == "true" {
        return Ok(());
    }

    let _ = Command::new("docker").args(["rm", "-f", CONTAINER]).output();

    let start = Command::new("docker")
        .args(["run", "-d", "--name", CONTAINER, IMAGE])
        .output()
        .map_err(|e| format!("cannot start container: {}", e))?;

    if !start.status.success() {
        let err = String::from_utf8_lossy(&start.stderr);
        return Err(format!("container start failed: {}", err));
    }

    Ok(())
}

pub fn exec_mkdir(uuid: &str) -> Result<(), String> {
    let out = Command::new("docker")
        .args(["exec", CONTAINER, "mkdir", "-p", &format!("/var/code/{}", uuid)])
        .output()
        .map_err(|e| format!("mkdir: {}", e))?;

    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).to_string());
    }
    Ok(())
}

pub fn copy_into(local_path: &str, container_path: &str) -> Result<(), String> {
    let dest = format!("{}:{}", CONTAINER, container_path);
    let out = Command::new("docker")
        .args(["cp", local_path, &dest])
        .output()
        .map_err(|e| format!("docker cp: {}", e))?;

    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).to_string());
    }
    Ok(())
}

pub fn exec_unzip(uuid: &str) -> Result<String, String> {
    let zip_path = format!("/tmp/{}.zip", uuid);
    let dest = format!("/var/code/{}", uuid);

    let out = Command::new("docker")
        .args(["exec", CONTAINER, "unzip", "-o", &zip_path, "-d", &dest])
        .output()
        .map_err(|e| format!("unzip: {}", e))?;

    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();

    let _ = Command::new("docker")
        .args(["exec", CONTAINER, "rm", "-f", &zip_path])
        .output();

    let code = out.status.code();
    if !out.status.success() && code != Some(1) {
        return Err(format!("{} {}", stdout, stderr).trim().to_string());
    }
    Ok(stdout)
}

pub fn exec_compile(uuid: &str, source_type: &str) -> (bool, i32, String) {
    let cmd = match source_type {
        "c" => format!(
            "cd /var/code/{} && mkdir -p bin && gcc *.c -o bin/output 2>&1",
            uuid
        ),
        "cpp" => format!(
            "cd /var/code/{} && mkdir -p bin && g++ *.cpp -o bin/output 2>&1",
            uuid
        ),
        "rust" => format!(
            "cd /var/code/{} && cargo build --release 2>&1 && mkdir -p bin && find target/release -maxdepth 1 -type f ! -name '*.d' -perm /111 -exec cp {{}} bin/ \\;",
            uuid
        ),
        other => return (false, 1, format!("unknown source type: {}", other)),
    };

    match Command::new("docker").args(["exec", CONTAINER, "sh", "-c", &cmd]).output() {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            let combined = if stderr.is_empty() { stdout } else { format!("{}\n{}", stdout, stderr) };
            let code = out.status.code().unwrap_or(1);
            (out.status.success(), code, combined)
        }
        Err(e) => (false, 1, format!("docker exec: {}", e)),
    }
}

pub fn exec_zip_output(uuid: &str) -> Result<Vec<u8>, String> {
    let bin_path = format!("/var/code/{}/bin", uuid);
    let zip_path = format!("/tmp/{}-out.zip", uuid);

    let check = Command::new("docker")
        .args(["exec", CONTAINER, "test", "-d", &bin_path])
        .output()
        .map_err(|e| format!("check bin dir: {}", e))?;

    if !check.status.success() {
        return Err("no output directory found".to_string());
    }

    let zip_out = Command::new("docker")
        .args(["exec", "-w", &bin_path, CONTAINER, "zip", "-r", &zip_path, "."])
        .output()
        .map_err(|e| format!("zip: {}", e))?;

    if !zip_out.status.success() {
        return Err(String::from_utf8_lossy(&zip_out.stderr).to_string());
    }

    let temp = std::env::temp_dir().join(format!("{}-out.zip", uuid));
    let temp_str = temp.to_string_lossy().to_string();

    let cp = Command::new("docker")
        .args(["cp", &format!("{}:{}", CONTAINER, zip_path), &temp_str])
        .output()
        .map_err(|e| format!("docker cp out: {}", e))?;

    if !cp.status.success() {
        return Err(String::from_utf8_lossy(&cp.stderr).to_string());
    }

    let bytes = std::fs::read(&temp).map_err(|e| format!("read zip: {}", e))?;

    let _ = std::fs::remove_file(&temp);
    let _ = Command::new("docker")
        .args(["exec", CONTAINER, "rm", "-f", &zip_path])
        .output();

    Ok(bytes)
}
