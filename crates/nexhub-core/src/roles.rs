//! YAML loader and validation for agent presets and roles.

use crate::model::SpawnSpec;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RoleError {
    #[error("preset not found: '{0}'")]
    PresetNotFound(String),

    #[error("role not found: '{0}'")]
    RoleNotFound(String),

    #[error("yaml parse error: {0}")]
    Yaml(String),

    #[error("invalid name '{0}': names must match ^[a-z0-9-]+$")]
    InvalidName(String),

    #[error("missing required field: '{0}'")]
    MissingField(String),

    #[error("invalid regex pattern '{0}': {1}")]
    InvalidRegex(String, String),

    #[error("unknown preset '{0}' referenced by role")]
    UnknownPreset(String),

    #[error("unknown variable '${{{0}}}'")]
    UnknownVariable(String),

    #[error("io error: {0}")]
    Io(String),
}

/// First message delivery strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FirstMessageKind {
    #[default]
    Role,
    None,
}

/// Preset configuration specifying how to start an agent process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preset {
    pub name: String,
    pub program: String,
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub add_dirs_flag: Option<String>,
    #[serde(default)]
    pub system_prompt_flag: Option<String>,
    #[serde(default)]
    pub first_message: FirstMessageKind,
    #[serde(default)]
    pub quota_patterns: Vec<String>,
    #[serde(default)]
    pub quota_reset_regex: Option<String>,
    #[serde(default = "default_idle_ms")]
    pub idle_ms: u64,
}

fn default_idle_ms() -> u64 {
    3000
}

/// Budget configuration for an agent role.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Budget {
    #[serde(default)]
    pub max_hours_per_day: Option<u32>,
}

/// Role configuration specifying what an agent does and its initial instructions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Role {
    pub name: String,
    pub title: String,
    pub preset: String,
    #[serde(default)]
    pub add_dirs: Vec<String>,
    pub prompt: String,
    #[serde(default)]
    pub budget: Option<Budget>,
}

/// Interpolation context providing variable substitutions for templates.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Context {
    pub repo: String,
    pub floor: String,
    pub floor_path: String,
    pub agent: String,
    pub data: String,
}

/// Known built-in preset definitions compiled into the binary.
pub const BUILTIN_AGY_AUTO: &str = include_str!("../../../presets/agy-auto.yaml");
pub const BUILTIN_AGY: &str = include_str!("../../../presets/agy.yaml");
pub const BUILTIN_CLAUDE: &str = include_str!("../../../presets/claude.yaml");
pub const BUILTIN_SHELL: &str = include_str!("../../../presets/shell.yaml");

/// Known built-in role definitions compiled into the binary.
pub const BUILTIN_CONDUCTOR: &str = include_str!("../../../roles/conductor.yaml");
pub const BUILTIN_FLOOR_BUILDER: &str = include_str!("../../../roles/floor-builder.yaml");
pub const BUILTIN_REVIEWER: &str = include_str!("../../../roles/reviewer.yaml");

static NAME_REGEX: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"^[a-z0-9-]+$").unwrap());

static VAR_REGEX: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"\$\{([^}]+)\}").unwrap());

/// Validates that all variable references in `s` are valid, and interpolates them using `ctx`.
pub fn interpolate_variables(s: &str, ctx: &Context) -> Result<String, RoleError> {
    for cap in VAR_REGEX.captures_iter(s) {
        let var_name = &cap[1];
        match var_name {
            "repo" | "floor" | "floor_path" | "agent" | "data" => {}
            other => return Err(RoleError::UnknownVariable(other.to_string())),
        }
    }

    let mut result = s.to_string();
    result = result.replace("${repo}", &ctx.repo);
    result = result.replace("${floor}", &ctx.floor);
    result = result.replace("${floor_path}", &ctx.floor_path);
    result = result.replace("${agent}", &ctx.agent);
    result = result.replace("${data}", &ctx.data);
    Ok(result)
}

/// Validates variable usage in a string without substituting values.
pub fn validate_variables_in_str(s: &str) -> Result<(), RoleError> {
    for cap in VAR_REGEX.captures_iter(s) {
        let var_name = &cap[1];
        match var_name {
            "repo" | "floor" | "floor_path" | "agent" | "data" => {}
            other => return Err(RoleError::UnknownVariable(other.to_string())),
        }
    }
    Ok(())
}

/// Validates preset schema rules: name format, non-empty program, valid regexes and variables.
pub fn validate_preset(preset: &Preset) -> Result<(), RoleError> {
    if !NAME_REGEX.is_match(&preset.name) {
        return Err(RoleError::InvalidName(preset.name.clone()));
    }
    if preset.program.trim().is_empty() {
        return Err(RoleError::MissingField("program".to_string()));
    }
    for pat in &preset.quota_patterns {
        Regex::new(pat).map_err(|e| RoleError::InvalidRegex(pat.clone(), e.to_string()))?;
    }
    if let Some(r) = &preset.quota_reset_regex {
        Regex::new(r).map_err(|e| RoleError::InvalidRegex(r.clone(), e.to_string()))?;
    }
    for arg in &preset.args {
        validate_variables_in_str(arg)?;
    }
    for val in preset.env.values() {
        validate_variables_in_str(val)?;
    }
    Ok(())
}

/// Validates role schema rules: name format, valid preset reference, and variables.
pub fn validate_role(role: &Role, available_presets: Option<&[&str]>) -> Result<(), RoleError> {
    if !NAME_REGEX.is_match(&role.name) {
        return Err(RoleError::InvalidName(role.name.clone()));
    }
    if role.title.trim().is_empty() {
        return Err(RoleError::MissingField("title".to_string()));
    }
    if let Some(presets) = available_presets {
        if !presets.contains(&role.preset.as_str()) {
            return Err(RoleError::UnknownPreset(role.preset.clone()));
        }
    }
    for dir in &role.add_dirs {
        validate_variables_in_str(dir)?;
    }
    validate_variables_in_str(&role.prompt)?;
    Ok(())
}

/// Parses and validates a Preset YAML string.
pub fn parse_preset_yaml(yaml: &str) -> Result<Preset, RoleError> {
    let preset: Preset = serde_yaml::from_str(yaml).map_err(|e| RoleError::Yaml(e.to_string()))?;
    validate_preset(&preset)?;
    Ok(preset)
}

/// Parses and validates a Role YAML string.
pub fn parse_role_yaml(yaml: &str, available_presets: Option<&[&str]>) -> Result<Role, RoleError> {
    let role: Role = serde_yaml::from_str(yaml).map_err(|e| RoleError::Yaml(e.to_string()))?;
    validate_role(&role, available_presets)?;
    Ok(role)
}

/// Resolves a preset by name according to lookup order:
/// 1. `<repo>/.nexhub/presets/<name>.yaml`
/// 2. `<data dir>/presets/<name>.yaml`
/// 3. Built-in presets.
pub fn load_preset(
    name: &str,
    repo_path: Option<&Path>,
    data_dir: Option<&Path>,
) -> Result<Preset, RoleError> {
    if let Some(repo) = repo_path {
        let path = repo
            .join(".nexhub")
            .join("presets")
            .join(format!("{name}.yaml"));
        if path.exists() {
            let content =
                std::fs::read_to_string(&path).map_err(|e| RoleError::Io(e.to_string()))?;
            return parse_preset_yaml(&content);
        }
    }

    if let Some(data) = data_dir {
        let path = data.join("presets").join(format!("{name}.yaml"));
        if path.exists() {
            let content =
                std::fs::read_to_string(&path).map_err(|e| RoleError::Io(e.to_string()))?;
            return parse_preset_yaml(&content);
        }
    }

    match name {
        "agy" => parse_preset_yaml(BUILTIN_AGY),
        "agy-auto" => parse_preset_yaml(BUILTIN_AGY_AUTO),
        "claude" => parse_preset_yaml(BUILTIN_CLAUDE),
        "shell" => parse_preset_yaml(BUILTIN_SHELL),
        _ => Err(RoleError::PresetNotFound(name.to_string())),
    }
}

/// Resolves a role by name according to lookup order:
/// 1. `<repo>/.nexhub/roles/<name>.yaml`
/// 2. `<data dir>/roles/<name>.yaml`
/// 3. Built-in roles.
pub fn load_role(
    name: &str,
    repo_path: Option<&Path>,
    data_dir: Option<&Path>,
    available_presets: Option<&[&str]>,
) -> Result<Role, RoleError> {
    if let Some(repo) = repo_path {
        let path = repo
            .join(".nexhub")
            .join("roles")
            .join(format!("{name}.yaml"));
        if path.exists() {
            let content =
                std::fs::read_to_string(&path).map_err(|e| RoleError::Io(e.to_string()))?;
            return parse_role_yaml(&content, available_presets);
        }
    }

    if let Some(data) = data_dir {
        let path = data.join("roles").join(format!("{name}.yaml"));
        if path.exists() {
            let content =
                std::fs::read_to_string(&path).map_err(|e| RoleError::Io(e.to_string()))?;
            return parse_role_yaml(&content, available_presets);
        }
    }

    match name {
        "conductor" => parse_role_yaml(BUILTIN_CONDUCTOR, available_presets),
        "floor-builder" => parse_role_yaml(BUILTIN_FLOOR_BUILDER, available_presets),
        "reviewer" => parse_role_yaml(BUILTIN_REVIEWER, available_presets),
        _ => Err(RoleError::RoleNotFound(name.to_string())),
    }
}

/// Constructs a `SpawnSpec` by combining a preset, an optional role, and variable substitutions.
pub fn build_spawn_spec(
    preset: &Preset,
    role: Option<&Role>,
    ctx: &Context,
) -> Result<SpawnSpec, RoleError> {
    let mut args: Vec<String> = Vec::new();
    for arg in &preset.args {
        args.push(interpolate_variables(arg, ctx)?);
    }

    if let Some(role) = role {
        if let Some(flag) = &preset.add_dirs_flag {
            for dir in &role.add_dirs {
                let resolved_dir = interpolate_variables(dir, ctx)?;
                args.push(flag.clone());
                args.push(resolved_dir);
            }
        }
    }

    let mut env = Vec::new();
    for (k, v) in &preset.env {
        let resolved_v = interpolate_variables(v, ctx)?;
        env.push((k.clone(), resolved_v));
    }

    let cwd = if !ctx.floor_path.is_empty() {
        PathBuf::from(&ctx.floor_path)
    } else if !ctx.repo.is_empty() {
        PathBuf::from(&ctx.repo)
    } else {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    };

    let program = if preset.program == "$SHELL" {
        std::env::var("SHELL").unwrap_or_else(|_| {
            #[cfg(windows)]
            {
                "powershell.exe".to_string()
            }
            #[cfg(not(windows))]
            {
                "/bin/sh".to_string()
            }
        })
    } else {
        preset.program.clone()
    };

    Ok(SpawnSpec {
        program,
        args,
        cwd,
        env,
        cols: 80,
        rows: 24,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtins_load_and_validate() {
        let p_agy = load_preset("agy", None, None).unwrap();
        assert_eq!(p_agy.name, "agy");

        let p_auto = load_preset("agy-auto", None, None).unwrap();
        assert_eq!(p_auto.name, "agy-auto");

        let p_claude = load_preset("claude", None, None).unwrap();
        assert_eq!(p_claude.name, "claude");

        let p_shell = load_preset("shell", None, None).unwrap();
        assert_eq!(p_shell.name, "shell");

        let known_presets = ["agy", "agy-auto", "claude", "shell"];
        let r_conductor = load_role("conductor", None, None, Some(&known_presets)).unwrap();
        assert_eq!(r_conductor.name, "conductor");

        let r_builder = load_role("floor-builder", None, None, Some(&known_presets)).unwrap();
        assert_eq!(r_builder.name, "floor-builder");

        let r_reviewer = load_role("reviewer", None, None, Some(&known_presets)).unwrap();
        assert_eq!(r_reviewer.name, "reviewer");
    }

    #[test]
    fn test_validation_error_unknown_key_in_preset() {
        let yaml = r#"
name: custom
program: custom_prog
args: []
unexpected_field: 123
"#;
        let err = parse_preset_yaml(yaml);
        assert!(matches!(err, Err(RoleError::Yaml(_))));
    }

    #[test]
    fn test_validation_error_unknown_key_in_role() {
        let yaml = r#"
name: custom-role
title: Custom Role
preset: agy
prompt: Do something
extra_bogus_key: true
"#;
        let err = parse_role_yaml(yaml, None);
        assert!(matches!(err, Err(RoleError::Yaml(_))));
    }

    #[test]
    fn test_validation_error_missing_name_or_program() {
        let yaml_no_name = r#"
program: custom_prog
args: []
"#;
        assert!(matches!(
            parse_preset_yaml(yaml_no_name),
            Err(RoleError::Yaml(_))
        ));

        let yaml_no_program = r#"
name: custom
args: []
"#;
        assert!(matches!(
            parse_preset_yaml(yaml_no_program),
            Err(RoleError::Yaml(_))
        ));
    }

    #[test]
    fn test_validation_error_args_not_a_list() {
        let yaml = r#"
name: custom
program: custom_prog
args: "not-a-list"
"#;
        assert!(matches!(parse_preset_yaml(yaml), Err(RoleError::Yaml(_))));
    }

    #[test]
    fn test_validation_error_invalid_regex_in_quota_patterns() {
        let yaml = r#"
name: custom
program: custom_prog
args: []
quota_patterns:
  - "[unclosed"
"#;
        let err = parse_preset_yaml(yaml);
        assert!(matches!(err, Err(RoleError::InvalidRegex(_, _))));
    }

    #[test]
    fn test_validation_error_role_unknown_preset() {
        let yaml = r#"
name: builder
title: Builder
preset: non-existent-preset
prompt: Do work
"#;
        let known = ["agy", "claude"];
        let err = parse_role_yaml(yaml, Some(&known));
        assert!(matches!(err, Err(RoleError::UnknownPreset(_))));
    }

    #[test]
    fn test_validation_error_unknown_variable() {
        let yaml = r#"
name: custom-role
title: Custom
preset: agy
prompt: Hello ${non_existent_var}
"#;
        let err = parse_role_yaml(yaml, None);
        assert!(matches!(err, Err(RoleError::UnknownVariable(ref v)) if v == "non_existent_var"));
    }

    #[test]
    fn test_variable_interpolation_and_build_spawn_spec() {
        let ctx = Context {
            repo: "/path/to/myrepo".to_string(),
            floor: "core".to_string(),
            floor_path: "/path/to/myrepo--core".to_string(),
            agent: "builder-1".to_string(),
            data: "/data/nexhub".to_string(),
        };

        let interp = interpolate_variables(
            "Agent ${agent} on floor ${floor} in ${repo} (data: ${data})",
            &ctx,
        )
        .unwrap();
        assert_eq!(
            interp,
            "Agent builder-1 on floor core in /path/to/myrepo (data: /data/nexhub)"
        );

        let preset = Preset {
            name: "test-preset".to_string(),
            program: "test_cli".to_string(),
            args: vec!["--repo".to_string(), "${repo}".to_string()],
            env: HashMap::from([("TARGET_AGENT".to_string(), "${agent}".to_string())]),
            add_dirs_flag: Some("--add-dir".to_string()),
            system_prompt_flag: None,
            first_message: FirstMessageKind::Role,
            quota_patterns: vec![],
            quota_reset_regex: None,
            idle_ms: 3000,
        };

        let role = Role {
            name: "test-role".to_string(),
            title: "Tester".to_string(),
            preset: "test-preset".to_string(),
            add_dirs: vec!["${repo}/src".to_string()],
            prompt: "Prompt for ${agent}".to_string(),
            budget: None,
        };

        let spec = build_spawn_spec(&preset, Some(&role), &ctx).unwrap();
        assert_eq!(spec.program, "test_cli");
        assert_eq!(
            spec.args,
            vec![
                "--repo",
                "/path/to/myrepo",
                "--add-dir",
                "/path/to/myrepo/src"
            ]
        );
        assert_eq!(spec.cwd, PathBuf::from("/path/to/myrepo--core"));
        assert!(spec
            .env
            .contains(&("TARGET_AGENT".to_string(), "builder-1".to_string())));
    }
}
