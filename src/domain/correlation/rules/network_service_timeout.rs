//! Network Down → Service Timeout correlation rule.
//!
//! Detects when a network connectivity loss is followed by service failures
//! that likely depend on network access (e.g., DNS failures, connection timeouts).

use crate::domain::correlation::rule::{time_proximity_confidence, Rule, RuleMatch, RuleMetadata};
use crate::domain::event::{Event, EventType};
use chrono::Duration;
use once_cell::sync::Lazy;

static METADATA: Lazy<RuleMetadata> = Lazy::new(|| RuleMetadata {
    id: "net-svc-timeout".to_string(),
    title: "Network Down → Service Timeout".to_string(),
    description: "Correlates network link-down or DHCP/DNS failures with subsequent \
                  service failures that likely depend on network access."
        .to_string(),
    priority: 3,
    time_window: Duration::minutes(10),
});

/// Rule that correlates network outages with service failures.
pub struct NetworkServiceTimeoutRule;

impl NetworkServiceTimeoutRule {
    /// Creates a new instance of this rule.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for NetworkServiceTimeoutRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for NetworkServiceTimeoutRule {
    fn metadata(&self) -> &RuleMetadata {
        &METADATA
    }

    fn find_matches(&self, events: &[Event]) -> Vec<RuleMatch> {
        let mut matches = Vec::new();

        // Network failure events
        let net_failures: Vec<_> = events
            .iter()
            .filter(|e| {
                matches!(
                    e.event_type,
                    EventType::NetworkLinkDown
                        | EventType::NetworkDhcpFailure
                        | EventType::NetworkDnsFailure
                )
            })
            .collect();

        // Service failures that may be network-dependent
        let svc_failures: Vec<_> = events
            .iter()
            .filter(|e| e.event_type == EventType::ServiceFailed)
            .collect();

        for net in &net_failures {
            let mut related_svcs = Vec::new();

            for svc in &svc_failures {
                let time_gap = svc.timestamp - net.timestamp;
                if time_gap < Duration::zero() || time_gap > METADATA.time_window {
                    continue;
                }

                // Check if the service failure message hints at network issues
                let msg = svc.summary.to_lowercase();
                let network_related = msg.contains("timeout")
                    || msg.contains("connection refused")
                    || msg.contains("network")
                    || msg.contains("dns")
                    || msg.contains("unreachable")
                    || msg.contains("timed out")
                    // Common network-dependent services
                    || svc.service.as_deref().map_or(false, |s| {
                        s.contains("ntp")
                            || s.contains("chrony")
                            || s.contains("sssd")
                            || s.contains("ldap")
                            || s.contains("kerberos")
                            || s.contains("docker")
                            || s.contains("podman")
                    });

                if network_related {
                    related_svcs.push(*svc);
                }
            }

            if related_svcs.is_empty() {
                continue;
            }

            let mut event_ids = vec![net.id];
            event_ids.extend(related_svcs.iter().map(|s| s.id));

            let confidence = time_proximity_confidence(
                related_svcs[0].timestamp - net.timestamp,
                METADATA.time_window,
                75,
            );

            let svc_names: Vec<_> = related_svcs
                .iter()
                .filter_map(|s| s.service.as_deref())
                .collect();

            let explanation = format!(
                "Network {} detected, followed by {} service failure(s): {}. \
                 These services likely failed due to loss of network connectivity.",
                net.event_type.label().to_lowercase(),
                related_svcs.len(),
                svc_names.join(", ")
            );

            matches.push(RuleMatch::new(event_ids, net.id, confidence, explanation));
        }

        matches
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::event::Severity;
    use chrono::Utc;

    #[test]
    fn test_network_service_timeout() {
        let now = Utc::now();

        let net_down = Event::new(
            now,
            EventType::NetworkLinkDown,
            Severity::Warning,
            "eth0: link down".to_string(),
        );

        let svc_fail = Event::new(
            now + Duration::seconds(30),
            EventType::ServiceFailed,
            Severity::Error,
            "sssd.service: connection timed out".to_string(),
        )
        .with_service("sssd.service");

        let rule = NetworkServiceTimeoutRule::new();
        let results = rule.find_matches(&[net_down, svc_fail]);
        assert_eq!(results.len(), 1);
    }
}
