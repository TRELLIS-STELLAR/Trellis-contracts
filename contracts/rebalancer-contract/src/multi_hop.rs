use crate::RebalancerError;
use soroban_sdk::{contracttype, Env, Symbol, Vec};

/// A caller-supplied execution receipt for one multi-hop route.
///
/// The contract does not perform AMM calls itself. The executor supplies the
/// quoted and observed output for each hop, and this module is the atomic
/// circuit-breaker boundary that decides whether the route may continue.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MultiHopSwap {
    pub amount_in: u128,
    pub route: Vec<Symbol>,
    pub quoted_outputs: Vec<u128>,
    pub actual_outputs: Vec<u128>,
    pub min_final_output: u128,
    pub max_slippage_bps: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MultiHopReport {
    pub amount_in: u128,
    pub amount_out: u128,
    pub total_slippage_bps: u32,
    pub hops: u32,
}

/// Validate a complete route before any downstream swap call is made.
///
/// A failed validation has no storage side effects, so the same receipt can
/// be retried with a fresh quote. Slippage is calculated conservatively from
/// the quoted output at each hop, and the final output floor is checked after
/// the last hop.
pub fn check_multi_hop(env: &Env, swap: &MultiHopSwap) -> Result<MultiHopReport, RebalancerError> {
    let route_len = swap.route.len();
    let hop_count = route_len.saturating_sub(1);
    if route_len < 2
        || swap.amount_in == 0
        || swap.min_final_output == 0
        || swap.max_slippage_bps > 10_000
        || swap.quoted_outputs.len() != hop_count
        || swap.actual_outputs.len() != hop_count
    {
        return Err(RebalancerError::InvalidMultiHopRoute);
    }

    let mut total_slippage_bps: u32 = 0;
    let mut previous_output = swap.amount_in;
    for index in 0..hop_count {
        let input_asset = swap.route.get(index).unwrap();
        let output_asset = swap.route.get(index + 1).unwrap();
        if input_asset == output_asset {
            return Err(RebalancerError::InvalidMultiHopRoute);
        }

        let quoted = swap.quoted_outputs.get(index).unwrap();
        let actual = swap.actual_outputs.get(index).unwrap();
        if quoted == 0 || actual == 0 || actual > quoted || quoted > previous_output {
            return Err(RebalancerError::InvalidMultiHopRoute);
        }

        let loss = quoted - actual;
        let hop_slippage = loss.saturating_mul(10_000).saturating_add(quoted - 1) / quoted;
        if hop_slippage > u128::from(swap.max_slippage_bps)
            || total_slippage_bps.saturating_add(hop_slippage as u32) > swap.max_slippage_bps
        {
            return Err(RebalancerError::SlippageLimitExceeded);
        }
        total_slippage_bps = total_slippage_bps.saturating_add(hop_slippage as u32);
        previous_output = actual;
    }

    if previous_output < swap.min_final_output {
        return Err(RebalancerError::MinimumOutputNotMet);
    }

    Ok(MultiHopReport {
        amount_in: swap.amount_in,
        amount_out: previous_output,
        total_slippage_bps,
        hops: hop_count as u32,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::symbol_short;

    fn valid_swap(env: &Env) -> MultiHopSwap {
        let mut route = Vec::new(env);
        route.push_back(symbol_short!("USDC"));
        route.push_back(symbol_short!("XLM"));
        route.push_back(symbol_short!("EURC"));
        let mut quoted = Vec::new(env);
        quoted.push_back(900);
        quoted.push_back(800);
        let mut actual = Vec::new(env);
        actual.push_back(890);
        actual.push_back(790);
        MultiHopSwap {
            amount_in: 1_000,
            route,
            quoted_outputs: quoted,
            actual_outputs: actual,
            min_final_output: 780,
            max_slippage_bps: 300,
        }
    }

    #[test]
    fn accepts_normal_route_and_reports_aggregate_slippage() {
        let env = Env::default();
        let report = check_multi_hop(&env, &valid_swap(&env)).unwrap();
        assert_eq!(report.amount_out, 790);
        assert_eq!(report.hops, 2);
        assert_eq!(report.total_slippage_bps, 237);
    }

    #[test]
    fn trips_on_one_bad_hop_without_mutating_receipt() {
        let env = Env::default();
        let mut swap = valid_swap(&env);
        swap.actual_outputs.set(1, 500);
        assert_eq!(
            check_multi_hop(&env, &swap),
            Err(RebalancerError::SlippageLimitExceeded)
        );
        swap.actual_outputs.set(1, 790);
        assert!(check_multi_hop(&env, &swap).is_ok());
    }

    #[test]
    fn rejects_route_with_missing_or_repeated_asset() {
        let env = Env::default();
        let mut swap = valid_swap(&env);
        swap.route.set(1, symbol_short!("USDC"));
        assert_eq!(
            check_multi_hop(&env, &swap),
            Err(RebalancerError::InvalidMultiHopRoute)
        );
    }

    #[test]
    fn rejects_when_final_floor_is_not_met() {
        let env = Env::default();
        let mut swap = valid_swap(&env);
        swap.min_final_output = 801;
        assert_eq!(
            check_multi_hop(&env, &swap),
            Err(RebalancerError::MinimumOutputNotMet)
        );
    }

    #[test]
    fn authorization_is_checked_before_route_validation() {
        let env = Env::default();
        let contract = env.register_contract(None, crate::MultiAssetRebalancer);
        let admin = soroban_sdk::Address::generate(&env);
        let outsider = soroban_sdk::Address::generate(&env);
        env.mock_all_auths();
        let client = crate::MultiAssetRebalancerClient::new(&env, &contract);
        client.initialize(&admin).unwrap();

        assert_eq!(
            client.try_execute_multi_hop(&outsider, &valid_swap(&env)),
            Ok(Err(RebalancerError::Unauthorized))
        );
        assert!(client.execute_multi_hop(&admin, &valid_swap(&env)).is_ok());
    }
}
