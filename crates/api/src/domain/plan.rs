use std::str::FromStr;

use chrono::{DateTime, Utc};

use crate::domain::error::DomainError;
use crate::domain::ids::{OrgId, PlanId, SubscriptionId};
use crate::domain::value_objects::{Money, Tier};

/// A pricing-catalog entry. Each machine carries one plan as its tier.
#[derive(Debug, Clone)]
pub struct Plan {
    pub id: PlanId,
    pub code: Tier,
    pub name: String,
    pub price_per_machine_year: Money,
    /// One-time fee, charged per machine on onboarding.
    pub onboarding_fee: Money,
    /// Feature flags as raw JSON text (e.g. {"live_data":true}).
    pub features: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

impl Plan {
    /// Does this plan grant the named feature? Reads the `features` JSON for a
    /// boolean key without pulling serde_json into the domain.
    ///
    /// Cheap substring check is acceptable here: feature keys are controlled,
    /// fixed strings ("live_data", "predict", "static_passport") and the JSON
    /// is produced by us, not arbitrary user input.
    pub fn allows(&self, feature: &str) -> bool {
        match &self.features {
            Some(json) => {
                let needle = format!("\"{feature}\"");
                match json.find(&needle) {
                    Some(idx) => {
                        let after = &json[idx + needle.len()..];
                        let after = after.trim_start_matches([':', ' ']);
                        after.starts_with("true")
                    }
                    None => false,
                }
            }
            None => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscriptionStatus {
    Trialing,
    Active,
    PastDue,
    Canceled,
}

impl SubscriptionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            SubscriptionStatus::Trialing => "trialing",
            SubscriptionStatus::Active => "active",
            SubscriptionStatus::PastDue => "past_due",
            SubscriptionStatus::Canceled => "canceled",
        }
    }
}

impl FromStr for SubscriptionStatus {
    type Err = DomainError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "trialing" => Ok(SubscriptionStatus::Trialing),
            "active" => Ok(SubscriptionStatus::Active),
            "past_due" => Ok(SubscriptionStatus::PastDue),
            "canceled" => Ok(SubscriptionStatus::Canceled),
            other => Err(DomainError::InvalidSubscriptionStatus(other.to_string())),
        }
    }
}

impl std::fmt::Display for SubscriptionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One subscription per organization: status + billing period.
#[derive(Debug, Clone)]
pub struct Subscription {
    pub id: SubscriptionId,
    pub organization_id: OrgId,
    pub status: SubscriptionStatus,
    pub trial_ends_at: Option<DateTime<Utc>>,
    pub current_period_start: Option<DateTime<Utc>>,
    pub current_period_end: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscription_status_round_trip() {
        for s in [
            SubscriptionStatus::Trialing,
            SubscriptionStatus::Active,
            SubscriptionStatus::PastDue,
            SubscriptionStatus::Canceled,
        ] {
            assert_eq!(s.as_str().parse::<SubscriptionStatus>().unwrap(), s);
        }
        assert!("bogus".parse::<SubscriptionStatus>().is_err());
    }

    #[test]
    fn plan_allows_reads_feature_flags() {
        let plan = Plan {
            id: PlanId::new(),
            code: Tier::Live,
            name: "Passport Live".to_string(),
            price_per_machine_year: Money::new(
                360_000,
                crate::domain::value_objects::Currency::inr(),
            ),
            onboarding_fee: Money::new(60_000, crate::domain::value_objects::Currency::inr()),
            features: Some(
                r#"{"static_passport": true, "live_data": true, "predict": false}"#.to_string(),
            ),
            is_active: true,
            created_at: Utc::now(),
        };
        assert!(plan.allows("live_data"));
        assert!(plan.allows("static_passport"));
        assert!(!plan.allows("predict"));
        assert!(!plan.allows("nonexistent"));
    }
}
