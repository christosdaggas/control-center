//! Severity badge widget.

use crate::domain::event::Severity;
use crate::i18n::tr;
use gtk4::prelude::*;
use gtk4::Label;

/// A badge displaying severity level.
pub struct SeverityBadge;

impl SeverityBadge {
    /// Creates a new severity badge.
    #[must_use]
    pub fn new(severity: Severity) -> Label {
        let label = Label::builder()
            .label(severity.label())
            .css_classes(vec![
                "severity-badge".to_string(),
                Self::severity_class(severity).to_string(),
            ])
            .build();


        label.update_property(&[
            gtk4::accessible::Property::Label(&format!("{}: {}", tr("Severity"), severity.label())),
        ]);

        label
    }

    fn severity_class(severity: Severity) -> &'static str {
        match severity {
            Severity::Info => "info",
            Severity::Warning => "warning",
            Severity::Error => "error",
            Severity::Critical => "critical",
        }
    }
}
