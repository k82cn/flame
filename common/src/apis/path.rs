use crate::FlameError;
use std::fmt;
use std::str::FromStr;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct ApplicationGID {
    pub workspace: String,
    pub application: String,
}

impl ApplicationGID {
    pub fn new(
        workspace: impl Into<String>,
        application: impl Into<String>,
    ) -> Result<Self, FlameError> {
        let gid = Self {
            workspace: workspace.into(),
            application: application.into(),
        };
        validate_path_segment(&gid.workspace)?;
        validate_path_segment(&gid.application)?;
        Ok(gid)
    }
}

impl FromStr for ApplicationGID {
    type Err = FlameError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (workspace, application) = parse_application_path(value)?;
        Self::new(workspace, application)
    }
}

impl fmt::Display for ApplicationGID {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.workspace, self.application)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct SessionGID {
    pub workspace: String,
    pub session: String,
}

impl SessionGID {
    pub fn new(
        workspace: impl Into<String>,
        session: impl Into<String>,
    ) -> Result<Self, FlameError> {
        let workspace = workspace.into();
        let session = session.into();
        validate_path_segment(&workspace)?;
        validate_session_name(&session)?;
        Ok(Self { workspace, session })
    }
}

impl FromStr for SessionGID {
    type Err = FlameError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (workspace, session) = parse_session_path(value)?;
        Self::new(workspace, session)
    }
}

impl fmt::Display for SessionGID {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.workspace, self.session)
    }
}

pub const DEFAULT_WORKSPACE: &str = "default";

pub fn validate_path_segment(segment: &str) -> Result<(), FlameError> {
    if segment.is_empty()
        || segment.len() > 253
        || segment.starts_with('.')
        || segment.starts_with('-')
        || segment.contains("..")
        || !segment
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(FlameError::InvalidConfig(format!(
            "invalid path segment <{segment}>"
        )));
    }
    Ok(())
}

pub fn validate_session_name(name: &str) -> Result<(), FlameError> {
    validate_path_segment(name)?;
    if matches!(name, "pkg" | "bootstrap" | "shared") {
        return Err(FlameError::InvalidConfig(format!(
            "session name <{name}> is reserved"
        )));
    }
    Ok(())
}

pub fn application_path(workspace: &str, application: &str) -> Result<String, FlameError> {
    Ok(ApplicationGID::new(workspace, application)?.to_string())
}

pub fn parse_application_path(path: &str) -> Result<(&str, &str), FlameError> {
    let (workspace, application) = path
        .split_once('/')
        .ok_or_else(|| FlameError::InvalidConfig("application path must be ws/app".into()))?;
    application_path(workspace, application)?;
    Ok((workspace, application))
}

pub fn normalize_application_path(path: &str) -> Result<String, FlameError> {
    if path.contains('/') {
        parse_application_path(path)?;
        Ok(path.to_string())
    } else {
        application_path(DEFAULT_WORKSPACE, path)
    }
}

pub fn resolve_application_path(name: &str, id: &str) -> Result<String, FlameError> {
    validate_path_segment(name)?;
    let path = if id.is_empty() {
        application_path(DEFAULT_WORKSPACE, name)?
    } else {
        normalize_application_path(id)?
    };
    let (_, path_name) = parse_application_path(&path)?;
    if path_name != name {
        return Err(FlameError::InvalidConfig(format!(
            "application path <{path}> does not match name <{name}>"
        )));
    }
    Ok(path)
}

pub fn session_path(workspace: &str, session: &str) -> Result<String, FlameError> {
    Ok(SessionGID::new(workspace, session)?.to_string())
}

pub fn parse_session_path(path: &str) -> Result<(&str, &str), FlameError> {
    let (workspace, session) = path
        .rsplit_once('/')
        .ok_or_else(|| FlameError::InvalidConfig("session path must be ws/ssn".into()))?;
    session_path(workspace, session)?;
    Ok((workspace, session))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_paths_and_default_workspace() {
        assert_eq!(normalize_application_path("app").unwrap(), "default/app");
        assert_eq!(application_path("team", "app").unwrap(), "team/app");
        assert_eq!(session_path("team", "run").unwrap(), "team/run");
        assert_eq!(parse_session_path("team/run").unwrap(), ("team", "run"));
        assert_eq!(
            "team/app".parse::<ApplicationGID>().unwrap().to_string(),
            "team/app"
        );
        assert_eq!(
            "team/run".parse::<SessionGID>().unwrap().to_string(),
            "team/run"
        );
        assert_eq!(
            "team/run/12"
                .parse::<crate::apis::TaskGID>()
                .unwrap()
                .to_string(),
            "team/run/12"
        );
    }

    #[test]
    fn invalid_and_reserved_components() {
        for segment in ["", ".", "..", ".x", "-x", "a/b", "a\\b", "a..b", "*"] {
            assert!(validate_path_segment(segment).is_err(), "{segment:?}");
        }
        for name in ["pkg", "bootstrap", "shared", "*"] {
            assert!(validate_session_name(name).is_err(), "{name:?}");
        }
        assert!(parse_application_path("team/app/extra").is_err());
        assert!(parse_session_path("team/run/extra").is_err());
        assert!("team/run/0".parse::<crate::apis::TaskGID>().is_err());
        assert!("team/app/run/12".parse::<crate::apis::TaskGID>().is_err());
    }
}
