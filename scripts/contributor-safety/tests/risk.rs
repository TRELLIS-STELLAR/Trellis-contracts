use contributor_safety_verification::{auth, errors::Error, risk_scoring::*};
use soroban_sdk::{contract, contractimpl, testutils::{Address as _, Ledger}, Address, Env};

#[contract]
struct Risk;

fn context(actor: Address, amount: i128, strikes: u32, batch: u32) -> ActionContext {
    ActionContext { action: HighImpactAction::TreasuryWithdrawal, actor, amount,
        max_reference_amount: i128::MAX, recipient_is_known: true,
        is_batch: batch > 1, batch_size: batch, prior_strikes: strikes }
}

#[contractimpl]
impl Risk {
    pub fn init(env: Env, admin: Address) { auth::set_admin(&env, &admin); }
    pub fn policy(env: Env, admin: Address, policy: RiskPolicy) -> Result<(), Error> {
        set_risk_policy(&env, &admin, HighImpactAction::TreasuryWithdrawal, &policy)
    }
    pub fn record(env: Env, actor: Address, amount: i128, strikes: u32, batch: u32) -> Result<RiskAssessment, Error> {
        assess_and_record(&env, &context(actor, amount, strikes, batch))
    }
    pub fn preview(env: Env, actor: Address, amount: i128, strikes: u32, batch: u32) -> RiskAssessment {
        evaluate_risk(&env, &context(actor, amount, strikes, batch))
    }
    pub fn profile(env: Env, actor: Address) -> Option<ActorRiskProfile> { get_actor_profile(&env, &actor) }
}

fn policy() -> RiskPolicy {
    RiskPolicy { block_threshold: 800, dual_auth_threshold: 500, throttle_threshold: 200,
        velocity_window_ledgers: 120, velocity_burst_limit: 2, dormancy_window_ledgers: 17_280, decay_per_window: 20 }
}

#[test]
fn extreme_amounts_and_counters_have_bounded_explanations() {
    let env = Env::default();
    let id = env.register_contract(None, Risk);
    let client = RiskClient::new(&env, &id);
    let actor = Address::generate(&env);
    // Preview is intentionally available without a signature.
    let result = client.preview(&actor, &i128::MAX, &u32::MAX, &u32::MAX);
    assert_eq!(result.total_score, 850);
    assert_eq!(result.verdict, RiskVerdict::Block);
    assert_eq!(result.factors.get(0).unwrap().observed, 10_000);
    assert_eq!(result.factors.iter().map(|f| f.score_weight).sum::<u32>(), result.total_score);
    assert_eq!(client.profile(&actor), None);
    let half = client.preview(&actor, &(i128::MAX / 2), &0, &1);
    assert_eq!(half.total_score, 0); // strictly below the 50% boundary
    let above = client.preview(&actor, &(i128::MAX / 2 + 1), &0, &1);
    assert_eq!(above.total_score, 175);
}

#[test]
fn cooldown_retry_and_failed_verdicts_preserve_velocity_history() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|l| l.sequence_number = 100);
    let id = env.register_contract(None, Risk);
    let client = RiskClient::new(&env, &id);
    let admin = Address::generate(&env);
    let actor = Address::generate(&env);
    client.init(&admin);
    client.policy(&admin, &policy());
    for _ in 0..2 { client.record(&actor, &1, &0, &1); }
    let before = client.profile(&actor);
    assert!(client.try_record(&actor, &1, &0, &1).is_err());
    assert_eq!(client.profile(&actor), before);
    env.ledger().with_mut(|l| l.sequence_number = 221);
    assert_eq!(client.record(&actor, &1, &0, &1).verdict, RiskVerdict::Allow);
    assert_eq!(client.profile(&actor).unwrap().total_actions_in_window, 1);
    let before = client.profile(&actor);
    assert!(client.try_record(&actor, &i128::MAX, &3, &30).is_err());
    assert!(client.try_record(&actor, &i128::MAX, &2, &1).is_err());
    assert!(client.try_record(&actor, &-1, &0, &1).is_err());
    assert_eq!(client.profile(&actor), before);
}

#[test]
fn unsigned_actor_and_non_admin_cannot_poison_history_or_policy() {
    let env = Env::default();
    let id = env.register_contract(None, Risk);
    let client = RiskClient::new(&env, &id);
    let actor = Address::generate(&env);
    let admin = Address::generate(&env);
    client.init(&admin);
    assert!(client.try_record(&actor, &1, &0, &1).is_err());
    assert!(client.try_policy(&actor, &policy()).is_err());
    assert_eq!(client.profile(&actor), None);
}

#[test]
fn invalid_policy_is_rejected_and_can_be_corrected() {
    let env = Env::default();
    env.mock_all_auths();
    let id = env.register_contract(None, Risk);
    let client = RiskClient::new(&env, &id);
    let admin = Address::generate(&env);
    client.init(&admin);
    let mut invalid = policy();
    invalid.velocity_burst_limit = 0;
    assert!(client.try_policy(&admin, &invalid).is_err());
    invalid = policy();
    invalid.throttle_threshold = 1001;
    assert!(client.try_policy(&admin, &invalid).is_err());
    client.policy(&admin, &policy());
}
