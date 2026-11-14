pub mod checkpoint;
pub mod config;
pub mod plan;
pub mod security;
pub mod stream_log;

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error at {path}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("TOML parse error in {path}")]
    TomlDe {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error(transparent)]
    TomlSer(#[from] toml::ser::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("path escapes project: {path}")]
    PathTraversal { path: PathBuf },
    #[error("invalid {field}: {message}")]
    Validation { field: String, message: String },
    #[error("security violation: {message}")]
    Security { message: String },
    #[error("keychain error")]
    Keychain(#[source] keyring_core::Error),
    #[error("plan parse error at line {line}: {message}")]
    PlanParse { line: usize, message: String },
    #[error("invalid agent report: {message}")]
    Report { message: String },
    #[error("agent {agent_id} failed: {message}")]
    Agent { agent_id: String, message: String },
    #[error("plan step {step_number}: {message}")]
    Engine { step_number: usize, message: String },
    #[error("interrupted")]
    Interrupted,
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use std::error::Error as StdError;
    use std::io;
    use std::path::PathBuf;

    use serde::{Deserialize, Serialize};

    use super::{Error, Result};

    #[derive(Debug, Deserialize)]
    struct SampleToml {
        _count: u32,
    }

    #[test]
    fn from_serde_json_error_converts_and_preserves_source() {
        struct FailingReader;
        impl io::Read for FailingReader {
            fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
                let root = Error::Io {
                    path: PathBuf::from("socket"),
                    source: io::Error::other("underlying socket closed"),
                };
                Err(io::Error::new(io::ErrorKind::BrokenPipe, root))
            }
        }

        fn parse_from_reader() -> Result<serde_json::Value> {
            Ok(serde_json::from_reader(FailingReader)?)
        }

        let err = parse_from_reader().unwrap_err();
        assert!(matches!(err, Error::Json(_)));
        let src = err
            .source()
            .expect("transparent Json variant preserves underlying io::Error source");
        assert_eq!(src.to_string(), "underlying socket closed");

        let syntax_err = serde_json::from_str::<serde_json::Value>("{invalid").unwrap_err();
        let expected_msg = syntax_err.to_string();
        let converted = Error::from(syntax_err);
        assert_eq!(converted.to_string(), expected_msg);
        assert!(converted.source().is_none());
    }

    #[test]
    fn from_toml_ser_error_converts_and_preserves_source() {
        struct FailingMapKey;
        impl Serialize for FailingMapKey {
            fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                use serde::ser::SerializeMap;
                let mut map = serializer.serialize_map(Some(1))?;
                map.serialize_entry(&[1u8, 2u8], "val")?;
                map.end()
            }
        }

        fn serialize_failing() -> Result<String> {
            Ok(toml::to_string(&FailingMapKey)?)
        }

        let direct_msg = toml::to_string(&FailingMapKey).unwrap_err().to_string();
        let err = serialize_failing().unwrap_err();
        assert!(matches!(err, Error::TomlSer(_)));
        assert_eq!(err.to_string(), direct_msg);
        // transparent delegates source() to inner error::source(), which is None for leaf toml::ser::Error
        assert!(err.source().is_none());
    }

    #[test]
    fn io_and_toml_de_preserve_source_chain() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "missing config");
        let wrapped_io = Error::Io {
            path: PathBuf::from("project.toml"),
            source: io_err,
        };
        let io_source = wrapped_io.source().expect("Io error must expose #[source]");
        assert_eq!(io_source.to_string(), "missing config");
        assert!(wrapped_io.to_string().contains("project.toml"));

        let de_err = toml::from_str::<SampleToml>("_count = \"not-a-number\"").unwrap_err();
        let de_msg = de_err.to_string();
        let wrapped_de = Error::TomlDe {
            path: PathBuf::from("project.toml"),
            source: de_err,
        };
        let de_source = wrapped_de
            .source()
            .expect("TomlDe error must expose #[source]");
        assert_eq!(de_source.to_string(), de_msg);
        assert!(wrapped_de.to_string().contains("project.toml"));
    }

    #[test]
    fn leaf_variants_have_no_source_and_format_fields() {
        let cases = [
            Error::PathTraversal {
                path: PathBuf::from("../outside"),
            },
            Error::Validation {
                field: "stages".into(),
                message: "must be 2..=15".into(),
            },
            Error::Security {
                message: "disallowed tool".into(),
            },
            Error::PlanParse {
                line: 12,
                message: "unclosed fence".into(),
            },
            Error::Report {
                message: "missing summary".into(),
            },
            Error::Agent {
                agent_id: "coder-1".into(),
                message: "timeout".into(),
            },
            Error::Engine {
                step_number: 3,
                message: "hash mismatch".into(),
            },
            Error::Interrupted,
        ];

        for err in &cases {
            assert!(err.source().is_none(), "expected no source for {err:?}");
            assert!(!err.to_string().is_empty());
        }
    }
}
