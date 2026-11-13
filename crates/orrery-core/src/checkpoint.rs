use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::config::{RoleConfig, RoleKind, validate_safe_relative_path};
use crate::{Error, Result};

pub const MAX_SUMMARY_BYTES: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReportStatus {
    Dispatch,
    StepPassed,
    Escalate,
    Complete,
    Blocked,
    Confirmed,
    Denied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubtaskAllocation {
    pub target_agent_id: String,
    pub instructions: String,
    #[serde(default)]
    pub write_scope: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentReport {
    pub status: ReportStatus,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_stage: Option<usize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allocations: Vec<SubtaskAllocation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feedback: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<String>,
}

impl AgentReport {
    pub fn validate_for_role(&self, role: &RoleConfig, stage_count: usize) -> Result<()> {
        let allowed = match role.kind {
            RoleKind::Orchestrator => matches!(
                self.status,
                ReportStatus::Dispatch | ReportStatus::StepPassed | ReportStatus::Escalate
            ),
            RoleKind::Implementer => {
                matches!(self.status, ReportStatus::Complete | ReportStatus::Blocked)
            }
            RoleKind::Validator => {
                matches!(self.status, ReportStatus::Confirmed | ReportStatus::Denied)
            }
        };
        if !allowed {
            return Err(Error::Report {
                message: format!(
                    "status {:?} is not allowed for role kind {:?}",
                    self.status, role.kind
                ),
            });
        }

        if self.summary.trim().is_empty() {
            return Err(Error::Report {
                message: "summary must not be empty".to_string(),
            });
        }
        if self.summary.len() > MAX_SUMMARY_BYTES {
            return Err(Error::Report {
                message: format!(
                    "summary length {} exceeds maximum of {MAX_SUMMARY_BYTES} bytes",
                    self.summary.len()
                ),
            });
        }

        if self.status == ReportStatus::Dispatch {
            match self.target_stage {
                Some(t) if (1..stage_count).contains(&t) => {}
                Some(t) => {
                    return Err(Error::Report {
                        message: format!("Dispatch target_stage {t} must be in 1..{stage_count}"),
                    });
                }
                None => {
                    return Err(Error::Report {
                        message: "Dispatch report requires target_stage".to_string(),
                    });
                }
            }

            if self.allocations.is_empty() {
                return Err(Error::Report {
                    message: "Dispatch report requires non-empty allocations".to_string(),
                });
            }

            for (idx, alloc) in self.allocations.iter().enumerate() {
                if alloc.target_agent_id.trim().is_empty() {
                    return Err(Error::Report {
                        message: format!("allocations[{idx}].target_agent_id must not be empty"),
                    });
                }
                if alloc.instructions.trim().is_empty() {
                    return Err(Error::Report {
                        message: format!("allocations[{idx}].instructions must not be empty"),
                    });
                }
                for scope in &alloc.write_scope {
                    validate_safe_relative_path(Path::new(scope), "write_scope").map_err(
                        |err| Error::Report {
                            message: format!("invalid write_scope '{scope}': {err}"),
                        },
                    )?;
                }
            }
        } else {
            if self.target_stage.is_some() {
                return Err(Error::Report {
                    message: "target_stage must be omitted when status is not Dispatch".to_string(),
                });
            }
            if !self.allocations.is_empty() {
                return Err(Error::Report {
                    message: "allocations must be empty when status is not Dispatch".to_string(),
                });
            }
        }

        if matches!(
            self.status,
            ReportStatus::Denied | ReportStatus::Blocked | ReportStatus::Escalate
        ) && self
            .feedback
            .as_deref()
            .is_none_or(|text| text.trim().is_empty())
        {
            return Err(Error::Report {
                message: format!("{:?} report requires non-empty feedback", self.status),
            });
        }

        Ok(())
    }
}

pub fn report_json_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["status", "summary"],
        "properties": {
            "status": {
                "type": "string",
                "enum": [
                    "Dispatch",
                    "StepPassed",
                    "Escalate",
                    "Complete",
                    "Blocked",
                    "Confirmed",
                    "Denied"
                ]
            },
            "summary": {
                "type": "string"
            },
            "target_stage": {
                "type": "integer",
                "minimum": 1
            },
            "allocations": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["target_agent_id", "instructions"],
                    "properties": {
                        "target_agent_id": { "type": "string" },
                        "instructions": { "type": "string" },
                        "write_scope": {
                            "type": "array",
                            "items": { "type": "string" }
                        }
                    }
                }
            },
            "feedback": {
                "type": "string"
            },
            "artifacts": {
                "type": "array",
                "items": { "type": "string" }
            }
        }
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StampedReport {
    pub step_number: usize,
    pub plan_step_sha256: String,
    pub round: usize,
    pub stage_index: usize,
    pub role_id: String,
    pub agent_id: String,
    pub recorded_at: chrono::DateTime<chrono::Utc>,
    pub report: AgentReport,
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use chrono::{TimeZone, Utc};

    use super::{
        AgentReport, MAX_SUMMARY_BYTES, ReportStatus, StampedReport, SubtaskAllocation,
        report_json_schema,
    };
    use crate::Error;
    use crate::config::{RoleConfig, RoleKind};

    fn role(kind: RoleKind) -> RoleConfig {
        let id = match kind {
            RoleKind::Orchestrator => "orchestrator",
            RoleKind::Implementer => "coder",
            RoleKind::Validator => "validator",
        };
        RoleConfig {
            role_id: id.to_string(),
            display_name: id.to_string(),
            kind,
            rules_file: format!("roles/{id}.md"),
            allowed_mcp_tools: Vec::new(),
        }
    }

    fn base_report(status: ReportStatus) -> AgentReport {
        let (target_stage, allocations) = if status == ReportStatus::Dispatch {
            (
                Some(1),
                vec![SubtaskAllocation {
                    target_agent_id: "coder-1".to_string(),
                    instructions: "implement feature".to_string(),
                    write_scope: vec!["src".to_string()],
                }],
            )
        } else {
            (None, Vec::new())
        };

        let feedback = if matches!(
            status,
            ReportStatus::Denied | ReportStatus::Blocked | ReportStatus::Escalate
        ) {
            Some("actionable feedback".to_string())
        } else {
            None
        };

        AgentReport {
            status,
            summary: "concise report summary".to_string(),
            target_stage,
            allocations,
            feedback,
            artifacts: Vec::new(),
        }
    }

    const ALL_STATUSES: [ReportStatus; 7] = [
        ReportStatus::Dispatch,
        ReportStatus::StepPassed,
        ReportStatus::Escalate,
        ReportStatus::Complete,
        ReportStatus::Blocked,
        ReportStatus::Confirmed,
        ReportStatus::Denied,
    ];

    #[test]
    fn orchestrator_statuses() {
        let r = role(RoleKind::Orchestrator);
        for status in ALL_STATUSES {
            let report = base_report(status);
            let res = report.validate_for_role(&r, 3);
            if matches!(
                status,
                ReportStatus::Dispatch | ReportStatus::StepPassed | ReportStatus::Escalate
            ) {
                assert!(res.is_ok(), "expected {status:?} to be allowed: {res:?}");
            } else {
                assert!(matches!(res, Err(Error::Report { .. })));
            }
        }
    }

    #[test]
    fn implementer_statuses() {
        let r = role(RoleKind::Implementer);
        for status in ALL_STATUSES {
            let report = base_report(status);
            let res = report.validate_for_role(&r, 3);
            if matches!(status, ReportStatus::Complete | ReportStatus::Blocked) {
                assert!(res.is_ok(), "expected {status:?} to be allowed: {res:?}");
            } else {
                assert!(matches!(res, Err(Error::Report { .. })));
            }
        }
    }

    #[test]
    fn validator_statuses() {
        let r = role(RoleKind::Validator);
        for status in ALL_STATUSES {
            let report = base_report(status);
            let res = report.validate_for_role(&r, 3);
            if matches!(status, ReportStatus::Confirmed | ReportStatus::Denied) {
                assert!(res.is_ok(), "expected {status:?} to be allowed: {res:?}");
            } else {
                assert!(matches!(res, Err(Error::Report { .. })));
            }
        }
    }

    #[test]
    fn summary_empty_or_too_long_rejected() {
        let r = role(RoleKind::Implementer);

        let mut blank = base_report(ReportStatus::Complete);
        blank.summary = "  ".to_string();
        assert!(matches!(
            blank.validate_for_role(&r, 3),
            Err(Error::Report { .. })
        ));

        let mut exact = base_report(ReportStatus::Complete);
        exact.summary = "a".repeat(MAX_SUMMARY_BYTES);
        assert!(exact.validate_for_role(&r, 3).is_ok());

        let mut too_long = base_report(ReportStatus::Complete);
        too_long.summary = "a".repeat(MAX_SUMMARY_BYTES + 1);
        assert!(matches!(
            too_long.validate_for_role(&r, 3),
            Err(Error::Report { .. })
        ));
    }

    #[test]
    fn dispatch_requires_target_and_allocations() {
        let r = role(RoleKind::Orchestrator);

        let mut missing_target = base_report(ReportStatus::Dispatch);
        missing_target.target_stage = None;
        assert!(matches!(
            missing_target.validate_for_role(&r, 3),
            Err(Error::Report { .. })
        ));

        let mut zero_target = base_report(ReportStatus::Dispatch);
        zero_target.target_stage = Some(0);
        assert!(matches!(
            zero_target.validate_for_role(&r, 3),
            Err(Error::Report { .. })
        ));

        let mut out_of_bounds_target = base_report(ReportStatus::Dispatch);
        out_of_bounds_target.target_stage = Some(3);
        assert!(matches!(
            out_of_bounds_target.validate_for_role(&r, 3),
            Err(Error::Report { .. })
        ));

        let mut empty_allocs = base_report(ReportStatus::Dispatch);
        empty_allocs.allocations.clear();
        assert!(matches!(
            empty_allocs.validate_for_role(&r, 3),
            Err(Error::Report { .. })
        ));

        let mut empty_agent = base_report(ReportStatus::Dispatch);
        empty_agent.allocations[0].target_agent_id = "  ".to_string();
        assert!(matches!(
            empty_agent.validate_for_role(&r, 3),
            Err(Error::Report { .. })
        ));

        let mut empty_instructions = base_report(ReportStatus::Dispatch);
        empty_instructions.allocations[0].instructions = "  ".to_string();
        assert!(matches!(
            empty_instructions.validate_for_role(&r, 3),
            Err(Error::Report { .. })
        ));
    }

    #[test]
    fn dispatch_rejects_bad_write_scope() {
        let r = role(RoleKind::Orchestrator);
        let mut report = base_report(ReportStatus::Dispatch);
        report.allocations[0].write_scope = vec!["../x".to_string()];
        assert!(matches!(
            report.validate_for_role(&r, 3),
            Err(Error::Report { .. })
        ));
    }

    #[test]
    fn non_dispatch_rejects_target_or_allocations() {
        let r = role(RoleKind::Orchestrator);

        let mut with_target = base_report(ReportStatus::StepPassed);
        with_target.target_stage = Some(1);
        assert!(matches!(
            with_target.validate_for_role(&r, 3),
            Err(Error::Report { .. })
        ));

        let mut with_allocs = base_report(ReportStatus::StepPassed);
        with_allocs.allocations = vec![SubtaskAllocation {
            target_agent_id: "coder-1".to_string(),
            instructions: "work".to_string(),
            write_scope: Vec::new(),
        }];
        assert!(matches!(
            with_allocs.validate_for_role(&r, 3),
            Err(Error::Report { .. })
        ));
    }

    #[test]
    fn feedback_required_for_denied_blocked_escalate() {
        let cases = [
            (RoleKind::Validator, ReportStatus::Denied),
            (RoleKind::Implementer, ReportStatus::Blocked),
            (RoleKind::Orchestrator, ReportStatus::Escalate),
        ];

        for (kind, status) in cases {
            let r = role(kind);

            let mut missing = base_report(status);
            missing.feedback = None;
            assert!(matches!(
                missing.validate_for_role(&r, 3),
                Err(Error::Report { .. })
            ));

            let mut whitespace = base_report(status);
            whitespace.feedback = Some("   ".to_string());
            assert!(matches!(
                whitespace.validate_for_role(&r, 3),
                Err(Error::Report { .. })
            ));

            let valid = base_report(status);
            assert!(valid.validate_for_role(&r, 3).is_ok());
        }
    }

    #[test]
    fn unknown_field_rejected() {
        let bad_report_toml = r#"
status = "Complete"
summary = "done"
unexpected_key = "nope"
"#;
        assert!(toml::from_str::<AgentReport>(bad_report_toml).is_err());

        let bad_alloc_toml = r#"
status = "Dispatch"
summary = "dispatching"
target_stage = 1

[[allocations]]
target_agent_id = "coder-1"
instructions = "do work"
extra = 123
"#;
        assert!(toml::from_str::<AgentReport>(bad_alloc_toml).is_err());
    }

    #[test]
    fn stamped_report_toml_round_trip() {
        let stamped = StampedReport {
            report: AgentReport {
                status: ReportStatus::Dispatch,
                summary: "dispatching stage 1".to_string(),
                target_stage: Some(1),
                allocations: vec![SubtaskAllocation {
                    target_agent_id: "local-coder".to_string(),
                    instructions: "implement checkpoint module".to_string(),
                    write_scope: vec!["crates/orrery-core/src".to_string()],
                }],
                feedback: None,
                artifacts: vec!["crates/orrery-core/src/checkpoint.rs".to_string()],
            },
            step_number: 7,
            plan_step_sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                .to_string(),
            round: 1,
            stage_index: 0,
            role_id: "orchestrator".to_string(),
            agent_id: "local-orch".to_string(),
            recorded_at: Utc.with_ymd_and_hms(2026, 11, 13, 18, 0, 0).unwrap(),
        };

        let encoded = toml::to_string(&stamped).unwrap();
        let decoded: StampedReport = toml::from_str(&encoded).unwrap();
        assert_eq!(decoded, stamped);
    }

    #[test]
    fn json_schema_required_matches_struct() {
        let schema = report_json_schema();
        assert_eq!(schema["type"], serde_json::json!("object"));
        assert_eq!(schema["additionalProperties"], serde_json::json!(false));
        assert_eq!(schema["required"], serde_json::json!(["status", "summary"]));

        let full_report = AgentReport {
            status: ReportStatus::Dispatch,
            summary: "full".to_string(),
            target_stage: Some(1),
            allocations: vec![SubtaskAllocation {
                target_agent_id: "agent-a".to_string(),
                instructions: "task".to_string(),
                write_scope: vec!["src".to_string()],
            }],
            feedback: Some("note".to_string()),
            artifacts: vec!["src/lib.rs".to_string()],
        };
        let serialized = serde_json::to_value(&full_report).unwrap();
        let struct_keys: BTreeSet<String> =
            serialized.as_object().unwrap().keys().cloned().collect();
        let schema_keys: BTreeSet<String> = schema["properties"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        assert_eq!(schema_keys, struct_keys);

        let status_enum = schema["properties"]["status"]["enum"].as_array().unwrap();
        assert_eq!(status_enum.len(), 7);

        let minimal = serde_json::json!({
            "status": "Complete",
            "summary": "all good"
        });
        let parsed: AgentReport = serde_json::from_value(minimal).unwrap();
        assert_eq!(parsed.status, ReportStatus::Complete);
        assert_eq!(parsed.summary, "all good");

        let missing_summary = serde_json::json!({
            "status": "Complete"
        });
        assert!(serde_json::from_value::<AgentReport>(missing_summary).is_err());
    }
}
