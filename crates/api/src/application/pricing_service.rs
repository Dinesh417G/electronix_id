//! Cost-estimate use case + the `tier_allows` feature gate.
//!
//! Recurring annual = Σ over `active` machines of their (active) plan's
//! `price_per_machine_year`. One-time = Σ `onboarding_fee` for machines created
//! within the current billing period (or all machines if no period is set).

use std::sync::Arc;

use crate::application::error::{AppResult, ApplicationError};
use crate::application::ports::machine_repo::MachineRepository;
use crate::application::ports::plan_repo::PlanRepository;
use crate::application::ports::subscription_repo::SubscriptionRepository;
use crate::domain::ids::{MachineId, OrgId};
use crate::domain::plan::{Plan, Subscription};
use crate::domain::value_objects::{Currency, Money, Tier};

#[derive(Debug, Clone)]
pub struct MachineLine {
    pub machine_id: MachineId,
    pub machine_name: String,
    pub plan_code: Option<Tier>,
    pub recurring: Money,
    pub onboarding: Money,
}

#[derive(Debug, Clone)]
pub struct Estimate {
    pub currency: Currency,
    pub lines: Vec<MachineLine>,
    pub recurring_total: Money,
    pub onboarding_total: Money,
}

#[derive(Clone)]
pub struct PricingService {
    machines: Arc<dyn MachineRepository>,
    plans: Arc<dyn PlanRepository>,
    subs: Arc<dyn SubscriptionRepository>,
}

impl PricingService {
    pub fn new(
        machines: Arc<dyn MachineRepository>,
        plans: Arc<dyn PlanRepository>,
        subs: Arc<dyn SubscriptionRepository>,
    ) -> Self {
        Self {
            machines,
            plans,
            subs,
        }
    }

    /// Future LIVE/PREDICT endpoints gate on this. Today nothing calls it for
    /// gating, but the helper is in place per §9.
    pub fn tier_allows(plan: &Plan, feature: &str) -> bool {
        plan.allows(feature)
    }

    /// The plan catalog, including roadmap (inactive) plans.
    pub async fn plans(&self) -> AppResult<Vec<Plan>> {
        self.plans.list(false).await
    }

    /// The organization's subscription record (status + billing period).
    pub async fn subscription(&self, org: OrgId) -> AppResult<Subscription> {
        self.subs
            .find_by_org(org)
            .await?
            .ok_or_else(|| ApplicationError::not_found("subscription"))
    }

    pub async fn estimate(&self, org: OrgId) -> AppResult<Estimate> {
        let machines = self.machines.list_all(org).await?;
        let plans = self.plans.list(false).await?; // include inactive to resolve any plan_id
        let sub = self.subs.find_by_org(org).await?;
        let period = sub.and_then(|s| s.current_period_start.zip(s.current_period_end));

        let currency = Currency::inr();
        let mut recurring_total = Money::zero(currency.clone());
        let mut onboarding_total = Money::zero(currency.clone());
        let mut lines = Vec::with_capacity(machines.len());

        for m in machines {
            let plan = m.plan_id.and_then(|pid| plans.iter().find(|p| p.id == pid));

            // Recurring only for active machines on an active, priced plan.
            let recurring = match plan {
                Some(p) if m.status.is_active() && p.is_active => p.price_per_machine_year.clone(),
                _ => Money::zero(currency.clone()),
            };

            // Onboarding for machines created within the current period.
            let in_period = match period {
                Some((start, end)) => m.created_at >= start && m.created_at <= end,
                None => true,
            };
            let onboarding = match plan {
                Some(p) if in_period => p.onboarding_fee.clone(),
                _ => Money::zero(currency.clone()),
            };

            recurring_total = recurring_total.add(&recurring)?;
            onboarding_total = onboarding_total.add(&onboarding)?;

            lines.push(MachineLine {
                machine_id: m.id,
                machine_name: m.name,
                plan_code: plan.map(|p| p.code),
                recurring,
                onboarding,
            });
        }

        Ok(Estimate {
            currency,
            lines,
            recurring_total,
            onboarding_total,
        })
    }
}
