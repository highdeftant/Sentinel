use std::{fs, process::Command};

use crate::models::{ServiceDetails, SystemdUnitState, UnitRef};

pub fn resolve_unit_for_pid(pid: u32) -> Option<UnitRef> {
    let cgroup_path = read_cgroup_path(pid)?;
    let (unit, is_user) = parse_unit_from_cgroup_line(&cgroup_path)?;

    Some(UnitRef {
        unit,
        is_user,
        cgroup_path,
    })
}

pub fn fetch_service_details(pid: u32, process_name: Option<String>) -> ServiceDetails {
    let mut warnings = Vec::new();
    let command_line = read_command_line(pid, &mut warnings);
    let unit_ref = resolve_unit_for_pid(pid);
    let cgroup_path = unit_ref.as_ref().map(|unit| unit.cgroup_path.clone());
    let resolved_unit = unit_ref.as_ref().map(|unit| unit.unit.clone());
    let resolved_unit_scope = unit_ref.as_ref().map(|unit| {
        if unit.is_user {
            "user".to_string()
        } else {
            "system".to_string()
        }
    });
    let unit_state = unit_ref
        .as_ref()
        .and_then(|unit| read_systemd_unit_state(unit, &mut warnings));
    let status_lines = unit_ref
        .as_ref()
        .map(|unit| read_systemctl_status(unit, &mut warnings))
        .unwrap_or_default();
    let recent_logs = unit_ref
        .as_ref()
        .map(|unit| read_recent_logs(unit, &mut warnings))
        .unwrap_or_default();

    ServiceDetails {
        pid,
        process_name,
        command_line,
        cgroup_path,
        resolved_unit,
        resolved_unit_scope,
        unit_state,
        status_lines,
        recent_logs,
        warnings,
    }
}

fn read_cgroup_path(pid: u32) -> Option<String> {
    let path = format!("/proc/{pid}/cgroup");
    let content = fs::read_to_string(path).ok()?;

    content
        .lines()
        .find(|line| parse_unit_from_cgroup_line(line).is_some())
        .or_else(|| content.lines().next())
        .map(str::to_string)
}

fn read_command_line(pid: u32, warnings: &mut Vec<String>) -> Option<String> {
    let path = format!("/proc/{pid}/cmdline");
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) => {
            warnings.push(format!("cmdline collector for pid {pid}: {error}"));
            return None;
        }
    };

    let parts = bytes
        .split(|byte| *byte == 0)
        .filter(|part| !part.is_empty())
        .map(|part| String::from_utf8_lossy(part).to_string())
        .collect::<Vec<_>>();

    if parts.is_empty() {
        return None;
    }

    Some(parts.join(" "))
}

fn read_systemd_unit_state(
    unit_ref: &UnitRef,
    warnings: &mut Vec<String>,
) -> Option<SystemdUnitState> {
    let mut command = Command::new("systemctl");
    if unit_ref.is_user {
        command.arg("--user");
    }

    let output = command
        .args([
            "show",
            unit_ref.unit.as_str(),
            "--property",
            "Id,Description,LoadState,ActiveState,SubState,FragmentPath,UnitFileState,MainPID,ExecMainPID",
            "--no-pager",
        ])
        .output();

    match output {
        Ok(output) if output.status.success() => {
            let text = String::from_utf8_lossy(&output.stdout);
            parse_systemctl_show_with_scope(&text, unit_ref.is_user)
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warnings.push(format!(
                "systemctl show {} exited with {}: {}",
                unit_ref.unit,
                output.status,
                stderr.trim()
            ));
            None
        }
        Err(error) => {
            warnings.push(format!("systemctl show {} failed: {error}", unit_ref.unit));
            None
        }
    }
}

fn read_recent_logs(unit_ref: &UnitRef, warnings: &mut Vec<String>) -> Vec<String> {
    let mut command = Command::new("journalctl");
    if unit_ref.is_user {
        command.arg("--user");
    }

    let output = command
        .args([
            "-u",
            unit_ref.unit.as_str(),
            "-n",
            "20",
            "--no-pager",
            "-o",
            "short-iso",
        ])
        .output();

    match output {
        Ok(output) if output.status.success() => String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect(),
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warnings.push(format!(
                "journalctl for {} exited with {}: {}",
                unit_ref.unit,
                output.status,
                stderr.trim()
            ));
            Vec::new()
        }
        Err(error) => {
            warnings.push(format!("journalctl for {} failed: {error}", unit_ref.unit));
            Vec::new()
        }
    }
}

fn read_systemctl_status(unit_ref: &UnitRef, warnings: &mut Vec<String>) -> Vec<String> {
    let mut command = Command::new("systemctl");
    if unit_ref.is_user {
        command.arg("--user");
    }

    let output = command
        .args([
            "status",
            unit_ref.unit.as_str(),
            "--no-pager",
            "--lines",
            "20",
        ])
        .output();

    match output {
        Ok(output) if output.status.success() || !output.stdout.is_empty() => {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                warnings.push(format!(
                    "systemctl status {} exited with {}: {}",
                    unit_ref.unit,
                    output.status,
                    stderr.trim()
                ));
            }

            String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(str::trim_end)
                .filter(|line| !line.is_empty())
                .map(str::to_string)
                .collect()
        }
        Ok(output) => {
            warnings.push(format!(
                "systemctl status {} exited with {}",
                unit_ref.unit, output.status
            ));
            Vec::new()
        }
        Err(error) => {
            warnings.push(format!(
                "systemctl status {} failed: {error}",
                unit_ref.unit
            ));
            Vec::new()
        }
    }
}

pub fn parse_unit_from_cgroup_line(line: &str) -> Option<(String, bool)> {
    let tail = line.rsplit(':').next()?;
    let segments = tail.split('/').collect::<Vec<_>>();
    let unit = segments
        .iter()
        .rfind(|segment| is_supported_unit_segment(segment))?;
    let is_user = segments
        .iter()
        .any(|segment| segment.starts_with("user@") && segment.ends_with(".service"));

    Some(((*unit).to_string(), is_user))
}

fn is_supported_unit_segment(segment: &&str) -> bool {
    segment.ends_with(".service") || segment.ends_with(".socket") || segment.ends_with(".scope")
}

#[cfg(test)]
pub fn parse_systemctl_show(text: &str) -> Option<SystemdUnitState> {
    parse_systemctl_show_with_scope(text, false)
}

fn parse_systemctl_show_with_scope(text: &str, is_user: bool) -> Option<SystemdUnitState> {
    let unit = field(text, "Id")?;
    let description = field(text, "Description").unwrap_or_default();
    let load_state = field(text, "LoadState").unwrap_or_default();
    let active_state = field(text, "ActiveState").unwrap_or_default();
    let sub_state = field(text, "SubState").unwrap_or_default();
    let fragment_path = field(text, "FragmentPath").filter(|value| !value.is_empty());
    let unit_file_state = field(text, "UnitFileState").filter(|value| !value.is_empty());
    let main_pid = field(text, "MainPID").and_then(|value| value.parse::<u32>().ok());

    Some(SystemdUnitState {
        unit,
        description,
        load_state,
        active_state,
        sub_state,
        fragment_path,
        unit_file_state,
        main_pid,
        is_user,
    })
}

fn field(text: &str, key: &str) -> Option<String> {
    text.lines()
        .find_map(|line| line.strip_prefix(&format!("{key}=")))
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::{SystemdUnitState, parse_systemctl_show, parse_unit_from_cgroup_line};

    #[test]
    fn parses_system_unit_name_from_cgroup() {
        let line = "0::/system.slice/ssh.service";
        assert_eq!(
            parse_unit_from_cgroup_line(line),
            Some(("ssh.service".to_string(), false))
        );
    }

    #[test]
    fn parses_user_unit_name_from_cgroup() {
        let line = "0::/user.slice/user-1000.slice/user@1000.service/app.slice/nextcloud-frankenphp-https.service";
        assert_eq!(
            parse_unit_from_cgroup_line(line),
            Some(("nextcloud-frankenphp-https.service".to_string(), true))
        );
    }

    #[test]
    fn keeps_session_scope_under_system_manager() {
        let line = "0::/user.slice/user-1000.slice/session-2.scope";
        assert_eq!(
            parse_unit_from_cgroup_line(line),
            Some(("session-2.scope".to_string(), false))
        );
    }

    #[test]
    fn parses_systemctl_show_payload() {
        let state = parse_systemctl_show(
            "Id=ssh.service\nDescription=OpenBSD Secure Shell server\nLoadState=loaded\nActiveState=active\nSubState=running\nFragmentPath=/usr/lib/systemd/system/ssh.service\nUnitFileState=enabled\nMainPID=991\nExecMainPID=991\n"
        )
        .expect("systemctl output should parse");

        assert_eq!(
            state,
            SystemdUnitState {
                unit: "ssh.service".to_string(),
                description: "OpenBSD Secure Shell server".to_string(),
                load_state: "loaded".to_string(),
                active_state: "active".to_string(),
                sub_state: "running".to_string(),
                fragment_path: Some("/usr/lib/systemd/system/ssh.service".to_string()),
                unit_file_state: Some("enabled".to_string()),
                main_pid: Some(991),
                is_user: false,
            }
        );
    }
}
