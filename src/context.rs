use std::env;
use std::path::Path;
use std::process::Command;

pub struct Context {
    pub os: String,
    pub shell: String,
    pub cwd: String,
    pub project: Option<String>,
}

impl Context {
    pub fn detect() -> Self {
        let cwd = env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        Self {
            os: detect_os(),
            shell: detect_shell(),
            project: detect_project(&cwd),
            cwd,
        }
    }

    pub fn as_prompt_context(&self) -> String {
        let project_tag = match &self.project {
            Some(p) => format!("\n  <project>{}</project>", p),
            None => String::new(),
        };
        format!(
            "<context>\n  <os>{}</os>\n  <shell>{}</shell>\n  <cwd>{}</cwd>{}\n</context>",
            self.os, self.shell, self.cwd, project_tag
        )
    }
}

fn detect_os() -> String {
    if cfg!(target_os = "macos") {
        "macOS".to_string()
    } else if cfg!(target_os = "linux") {
        "Linux".to_string()
    } else if cfg!(target_os = "windows") {
        "Windows".to_string()
    } else {
        "Unknown".to_string()
    }
}

fn detect_shell() -> String {
    if let Ok(shell_path) = env::var("SHELL") {
        if let Some(shell_name) = shell_path.split('/').last() {
            return shell_name.to_string();
        }
    }

    if cfg!(target_os = "windows") {
        if env::var("PSModulePath").is_ok() {
            return "powershell".to_string();
        }
        return "cmd".to_string();
    }

    if let Ok(output) = Command::new("ps")
        .args(["-p", &std::process::id().to_string(), "-o", "ppid="])
        .output()
    {
        if let Ok(ppid) = String::from_utf8_lossy(&output.stdout).trim().parse::<u32>() {
            if let Ok(output) = Command::new("ps")
                .args(["-p", &ppid.to_string(), "-o", "comm="])
                .output()
            {
                let comm = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !comm.is_empty() {
                    return comm.split('/').last().unwrap_or(&comm).to_string();
                }
            }
        }
    }

    "unknown".to_string()
}

fn detect_project(cwd: &str) -> Option<String> {
    let mut dir = Path::new(cwd);

    loop {
        if dir.join("Cargo.toml").exists() {
            return Some("rust".to_string());
        }
        if dir.join("package.json").exists() {
            if dir.join("tsconfig.json").exists() {
                return Some("typescript".to_string());
            }
            return Some("javascript".to_string());
        }
        if dir.join("go.mod").exists() {
            return Some("go".to_string());
        }
        if dir.join("pyproject.toml").exists() || dir.join("requirements.txt").exists() {
            return Some("python".to_string());
        }
        if dir.join("pom.xml").exists() {
            return Some("java".to_string());
        }
        if dir.join("build.gradle").exists() || dir.join("build.gradle.kts").exists() {
            return Some("java/kotlin".to_string());
        }

        match dir.parent() {
            Some(parent) if parent != dir => dir = parent,
            _ => break,
        }
    }

    None
}
