use crate::Span;
use miette::{Diagnostic as MietteDiagnostic, LabeledSpan, Severity as MietteSeverity, SourceSpan};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Severity {
    Error,
    Warning,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub span: Span,
}
impl Diagnostic {
    pub fn error(message: impl Into<String>, span: Span) -> Self {
        Self {
            severity: Severity::Error,
            message: message.into(),
            span,
        }
    }
}
impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for Diagnostic {}
impl MietteDiagnostic for Diagnostic {
    fn severity(&self) -> Option<MietteSeverity> {
        Some(match self.severity {
            Severity::Error => MietteSeverity::Error,
            Severity::Warning => MietteSeverity::Warning,
        })
    }
    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        Some(Box::new(std::iter::once(LabeledSpan::underline(SourceSpan::from((
            self.span.start,
            self.span.end.saturating_sub(self.span.start),
        ))))))
    }
}
