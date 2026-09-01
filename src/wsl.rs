use anyhow::{bail, Result};

pub fn parse_wsl_list_output(bytes: &[u8]) -> Vec<String> {
    let text = if looks_like_utf16_le(bytes) {
        let units = bytes
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .filter(|unit| *unit != 0xfeff)
            .collect::<Vec<_>>();
        String::from_utf16_lossy(&units)
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    };
    let mut distributions = Vec::new();
    for line in text.lines() {
        let distribution = line.trim().trim_matches('\0').trim();
        if distribution.is_empty()
            || is_system_distribution(distribution)
            || distributions
                .iter()
                .any(|value: &String| value.eq_ignore_ascii_case(distribution))
        {
            continue;
        }
        distributions.push(distribution.to_string());
    }
    distributions
}

fn looks_like_utf16_le(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0xff, 0xfe])
        || (bytes.len() >= 4
            && bytes
                .chunks_exact(2)
                .take(32)
                .filter(|pair| pair[1] == 0)
                .count()
                >= 2)
}

pub fn is_system_distribution(distribution: &str) -> bool {
    matches!(
        distribution.trim().to_ascii_lowercase().as_str(),
        "docker-desktop"
            | "docker-desktop-data"
            | "podman-machine-default"
            | "rancher-desktop"
            | "rancher-desktop-data"
    )
}

pub fn validate_distribution(value: &str) -> Result<()> {
    validate_name("distribution", value)
}

pub fn validate_user(value: &str) -> Result<()> {
    validate_name("user", value)
}

fn validate_name(label: &str, value: &str) -> Result<()> {
    let valid = !value.trim().is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'));
    if !valid {
        bail!("invalid WSL {label}: {value}");
    }
    Ok(())
}

pub fn validate_linux_absolute_path(label: &str, value: &str) -> Result<()> {
    if !value.starts_with('/') {
        bail!("WSL {label} must be an absolute Linux path");
    }
    if value.contains('\0')
        || value.contains('\t')
        || value.contains('\r')
        || value.contains('\n')
        || value.split('/').any(|segment| segment == "..")
    {
        bail!("WSL {label} contains an unsafe path segment");
    }
    Ok(())
}

pub fn is_wsl_mounted_path(value: &str) -> bool {
    let normalized = value.trim().replace('\\', "/");
    normalized == "/mnt" || normalized.starts_with("/mnt/")
}

pub fn is_wsl_unc_path(value: &str) -> bool {
    let normalized = value.trim().replace('\\', "/").to_ascii_lowercase();
    let normalized = normalized
        .strip_prefix("//?/unc/")
        .map(|rest| format!("//{rest}"))
        .unwrap_or(normalized);
    normalized == "//wsl.localhost"
        || normalized.starts_with("//wsl.localhost/")
        || normalized == "//wsl$"
        || normalized.starts_with("//wsl$/")
}

pub fn normalize_architecture(value: &str) -> Result<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "x86_64" | "amd64" => Ok("x86_64".to_string()),
        "aarch64" | "arm64" => Ok("aarch64".to_string()),
        other => bail!("unsupported WSL helper architecture: {other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_utf16_wsl_list_and_skips_system_distributions() {
        let text = "\u{feff}Ubuntu\r\ndocker-desktop\r\nDebian\r\n";
        let bytes = text
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        assert_eq!(parse_wsl_list_output(&bytes), vec!["Ubuntu", "Debian"]);
    }

    #[test]
    fn validates_names_paths_and_architecture() {
        assert!(validate_distribution("Ubuntu-24.04").is_ok());
        assert!(validate_user("dev_user").is_ok());
        assert!(validate_distribution("bad/name").is_err());
        assert!(validate_linux_absolute_path("Codex home", "/home/dev/.codex").is_ok());
        assert!(validate_linux_absolute_path("Codex home", "../.codex").is_err());
        assert!(is_wsl_mounted_path("/mnt"));
        assert!(is_wsl_mounted_path("/mnt/c/Users/dev/.codex"));
        assert!(!is_wsl_mounted_path("C:/mnt/c/Users/dev/.codex"));
        assert!(is_wsl_unc_path(r"\\wsl.localhost"));
        assert!(is_wsl_unc_path(r"\\?\UNC\wsl$\Ubuntu\home\dev\.codex"));
        assert!(!is_wsl_unc_path(r"\\server\share\.codex"));
        assert_eq!(normalize_architecture("amd64").unwrap(), "x86_64");
        assert_eq!(normalize_architecture("x86_64").unwrap(), "x86_64");
        assert_eq!(normalize_architecture("ARM64").unwrap(), "aarch64");
        assert_eq!(normalize_architecture("aarch64").unwrap(), "aarch64");
        assert!(normalize_architecture("armv7").is_err());
    }
}
