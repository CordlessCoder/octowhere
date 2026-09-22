const SYMBOL_PERIOD_OPTIMIZATION_THRESHOLD: f32 = 16.0;

/// See: datasheet page 31
pub(crate) fn should_optimize_for_low_data_rate(symbol_period: f32) -> bool {
    symbol_period > SYMBOL_PERIOD_OPTIMIZATION_THRESHOLD
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_optimize_false() {
        assert!(!should_optimize_for_low_data_rate(
            SYMBOL_PERIOD_OPTIMIZATION_THRESHOLD - 0.1
        ));
    }

    #[test]
    fn should_optimize_true() {
        assert!(should_optimize_for_low_data_rate(
            SYMBOL_PERIOD_OPTIMIZATION_THRESHOLD + 0.1
        ));
    }
}
