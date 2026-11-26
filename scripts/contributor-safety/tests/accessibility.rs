use contributor_safety_verification::accessibility::*;
use contributor_safety_verification::errors::Error;
use soroban_sdk::{contract, contractimpl, symbol_short, testutils::Address as _, Address, Env};

#[contract]
struct Audit;

#[contractimpl]
impl Audit {
    pub fn record(env: Env, caller: Address, metadata: AccessibilityMetadata) -> Result<(), Error> {
        record_accessibility_state(&env, &caller, &metadata)
    }

    pub fn read(env: Env) -> Option<AccessibilityMetadata> {
        get_last_accessibility_state(&env)
    }
}

#[test]
fn keyboard_and_announcements_cover_every_async_state() {
    use AsyncStatusState::*;
    for state in [Idle, Validating, AwaitingSignature, Broadcasting, Succeeded, RecoverableError, FatalError] {
        let metadata = map_state_to_accessibility(state, symbol_short!("submit"));
        let modal = matches!(state, AwaitingSignature | RecoverableError | FatalError);
        assert_eq!(metadata.focus_trap.trap_active, modal);
        assert_eq!(metadata.keyboard_mode, if modal { KeyboardNavMode::ModalTrap } else { KeyboardNavMode::Standard });
        assert_eq!(metadata.focus_trap.return_focus, symbol_short!("submit"));
        assert_eq!(metadata.focus_trap.dismissable, state != Broadcasting);
        assert_eq!(metadata.aria_busy, matches!(state, Validating | AwaitingSignature | Broadcasting));
        assert_eq!(metadata.announce_on_change, state != Idle);
        assert!(!metadata.animation_enabled);
        assert!(validate_focus_trap_config(&metadata.focus_trap).is_ok());
        if matches!(state, RecoverableError | FatalError) {
            assert_eq!(metadata.aria_live, AriaLiveMode::Assertive);
            assert_eq!(metadata.role, AriaRole::Alert);
        }
    }
}

#[test]
fn transition_matrix_rejects_skipped_signatures_and_terminal_retries() {
    use AsyncStatusState::*;
    let states = [Idle, Validating, AwaitingSignature, Broadcasting, Succeeded, RecoverableError, FatalError];
    let allowed = [(Idle, Validating), (Validating, AwaitingSignature), (Validating, RecoverableError),
        (Validating, FatalError), (AwaitingSignature, Broadcasting), (AwaitingSignature, RecoverableError),
        (AwaitingSignature, FatalError), (Broadcasting, Succeeded), (Broadcasting, RecoverableError),
        (Broadcasting, FatalError), (RecoverableError, Validating), (RecoverableError, Idle),
        (Succeeded, Idle), (FatalError, Idle)];
    for from in states {
        for to in states {
            assert_eq!(from.require_transition(to).is_ok(), allowed.contains(&(from, to)));
        }
    }
    let retry = map_error_recovery(RecoverableError, symbol_short!("submit"));
    assert_eq!(retry.focus_target, symbol_short!("retry_btn"));
    assert_eq!(retry.keyboard_mode, KeyboardNavMode::RovingTabindex);
    let fatal = map_error_recovery(FatalError, symbol_short!("support"));
    assert_eq!(fatal.focus_target, symbol_short!("support"));
    assert_eq!(fatal.keyboard_mode, KeyboardNavMode::LinearTab);
}

#[test]
fn authorized_retry_preserves_last_good_audit_after_invalid_input() {
    let env = Env::default();
    let id = env.register_contract(None, Audit);
    let client = AuditClient::new(&env, &id);
    let caller = Address::generate(&env);
    env.mock_all_auths();
    let failed = map_state_to_accessibility(AsyncStatusState::RecoverableError, symbol_short!("submit"));
    client.record(&caller, &failed);
    let mut invalid = failed.clone();
    invalid.focus_trap.return_focus = symbol_short!("");
    assert!(client.try_record(&caller, &invalid).is_err());
    assert_eq!(client.read(), Some(failed));
    let retry = map_state_to_accessibility(AsyncStatusState::Validating, symbol_short!("submit"));
    client.record(&caller, &retry);
    assert_eq!(client.read(), Some(retry));
}

#[test]
fn missing_signature_cannot_write_audit_state() {
    let env = Env::default();
    let id = env.register_contract(None, Audit);
    let client = AuditClient::new(&env, &id);
    let caller = Address::generate(&env);
    let metadata = map_state_to_accessibility(AsyncStatusState::AwaitingSignature, symbol_short!("submit"));
    assert!(client.try_record(&caller, &metadata).is_err());
    assert_eq!(client.read(), None);
}
