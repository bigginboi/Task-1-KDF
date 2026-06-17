use std::process::Command;

const CONTAINER: &str = "build-container";
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
        .args(["run", "-d", "--cpus", "2", "--memory", "2g", "--network", "none", "--name", CONTAINER, IMAGE, "sleep", "infinity"])
        .output()
        .map_err(|e| format!("cannot start container: {}", e))?;

    if !start.status.success() {
        let err = String::from_utf8_lossy(&start.stderr);
        return Err(format!("container start failed: {}", err));
    }

    Ok(())
}

pub fn copy_workspace(uuid: &str) -> Result<(), String> {
    let local_path = format!("./workspace/{}", uuid);
    let dest = format!("{}:/var/code/", CONTAINER);

    let _ = Command::new("docker")
        .args(["exec", CONTAINER, "mkdir", "-p", "/var/code"])
        .output();

    let out = Command::new("docker")
        .args(["cp", &local_path, &dest])
        .output()
        .map_err(|e| format!("docker cp: {}", e))?;

    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).to_string());
    }
    Ok(())
}

pub fn exec_compile(uuid: &str, source_type: &str) -> (bool, i32, String) {
    let compile_cmd = match source_type {
        "c" => "mkdir -p output && ( gcc $(find . -name '*.c' -not -path './output/*' -not -path '*/target/*') -o output/main ) > output/build.log 2>&1; STATUS=$?; echo \"Exit code: $STATUS\" >> output/build.log; exit $STATUS",
        "cpp" => "mkdir -p output && ( g++ $(find . \\( -name '*.cpp' -o -name '*.cc' -o -name '*.cxx' \\) -not -path './output/*' -not -path '*/target/*') -o output/main ) > output/build.log 2>&1; STATUS=$?; echo \"Exit code: $STATUS\" >> output/build.log; exit $STATUS",
        "rust" => "mkdir -p output && ( cargo build --release && find target/release -maxdepth 1 -type f -perm /111 -exec cp {} output/main \\; ) > output/build.log 2>&1; STATUS=$?; echo \"Exit code: $STATUS\" >> output/build.log; exit $STATUS",
        other => return (false, 1, format!("unknown source type: {}", other)),
    };

    let out = Command::new("docker")
        .args([
            "exec",
            "-w",
            &format!("/var/code/{}", uuid),
            CONTAINER,
            "timeout",
            "300s",
            "bash",
            "-c",
            compile_cmd,
        ])
        .output();

    match out {
        Ok(output) => {
            let status_code = output.status.code().unwrap_or(1);
            let success = output.status.success();

            let log_out = Command::new("docker")
                .args([
                    "exec",
                    CONTAINER,
                    "cat",
                    &format!("/var/code/{}/output/build.log", uuid),
                ])
                .output();

            let log_str = match log_out {
                Ok(log_output) if log_output.status.success() => {
                    String::from_utf8_lossy(&log_output.stdout).to_string()
                }
                _ => {
                    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                    if stderr.is_empty() { stdout } else { format!("{}\n{}", stdout, stderr) }
                }
            };

            (success, status_code, log_str)
        }
        Err(e) => (false, 1, format!("docker exec failed: {}", e)),
    }
}

pub fn exec_zip_output(uuid: &str) -> Result<Vec<u8>, String> {
    let out_dir = format!("/var/code/{}/output", uuid);

    let check = Command::new("docker")
        .args(["exec", CONTAINER, "test", "-d", &out_dir])
        .output()
        .map_err(|e| format!("check output dir: {}", e))?;

    if !check.status.success() {
        return Err("WORKSPACE_NOT_FOUND".to_string());
    }

    let zip_out = Command::new("docker")
        .args([
            "exec",
            "-w",
            &out_dir,
            CONTAINER,
            "zip",
            "-r",
            "-",
            ".",
        ])
        .output()
        .map_err(|e| format!("zip output: {}", e))?;

    if !zip_out.status.success() {
        return Err(String::from_utf8_lossy(&zip_out.stderr).to_string());
    }

    Ok(zip_out.stdout)
}

pub fn exec_cleanup(uuid: &str) -> Result<(), String> {
    let out = Command::new("docker")
        .args(["exec", CONTAINER, "rm", "-rf", &format!("/var/code/{}", uuid)])
        .output()
        .map_err(|e| format!("docker rm: {}", e))?;

    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).to_string());
    }
    Ok(())
}
