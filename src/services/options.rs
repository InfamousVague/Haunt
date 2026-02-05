//! Options Service
//!
//! Handles options pricing and Greeks calculations:
//! - Black-Scholes pricing for European options
//! - Binomial pricing for American options
//! - Greeks calculation (Delta, Gamma, Theta, Vega, Rho)
//! - Implied volatility calculation

#![allow(dead_code)]

use crate::types::{Greeks, OptionContract, OptionPosition, OptionStyle, OptionType};
use std::f64::consts::{E, PI};
use thiserror::Error;
use tracing::debug;

/// Options service errors.
#[derive(Debug, Error)]
pub enum OptionsError {
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error("Calculation failed: {0}")]
    CalculationError(String),
    #[error("IV convergence failed after {0} iterations")]
    IVConvergenceFailed(u32),
}

/// Options pricing and Greeks calculator.
pub struct OptionsService {
    /// Risk-free interest rate (annual, as decimal)
    risk_free_rate: f64,
}

impl OptionsService {
    /// Create a new options service with the given risk-free rate.
    pub fn new(risk_free_rate: f64) -> Self {
        Self { risk_free_rate }
    }

    /// Create with default 5% risk-free rate.
    pub fn default() -> Self {
        Self::new(0.05)
    }

    /// Set the risk-free interest rate.
    pub fn set_risk_free_rate(&mut self, rate: f64) {
        self.risk_free_rate = rate;
    }

    // ========== Black-Scholes Pricing ==========

    /// Calculate d1 parameter for Black-Scholes.
    fn d1(&self, spot: f64, strike: f64, time: f64, volatility: f64) -> f64 {
        let r = self.risk_free_rate;
        (f64::ln(spot / strike) + (r + volatility.powi(2) / 2.0) * time) / (volatility * time.sqrt())
    }

    /// Calculate d2 parameter for Black-Scholes.
    fn d2(&self, d1: f64, volatility: f64, time: f64) -> f64 {
        d1 - volatility * time.sqrt()
    }

    /// Standard normal cumulative distribution function.
    fn norm_cdf(&self, x: f64) -> f64 {
        // Approximation using error function
        0.5 * (1.0 + erf(x / 2.0_f64.sqrt()))
    }

    /// Standard normal probability density function.
    fn norm_pdf(&self, x: f64) -> f64 {
        E.powf(-x.powi(2) / 2.0) / (2.0 * PI).sqrt()
    }

    /// Calculate Black-Scholes price for a European option.
    pub fn black_scholes_price(
        &self,
        spot: f64,
        strike: f64,
        time_years: f64,
        volatility: f64,
        option_type: OptionType,
    ) -> Result<f64, OptionsError> {
        if spot <= 0.0 || strike <= 0.0 || time_years <= 0.0 || volatility <= 0.0 {
            return Err(OptionsError::InvalidInput(
                "All inputs must be positive".to_string(),
            ));
        }

        let d1 = self.d1(spot, strike, time_years, volatility);
        let d2 = self.d2(d1, volatility, time_years);
        let r = self.risk_free_rate;
        let discount = E.powf(-r * time_years);

        let price = match option_type {
            OptionType::Call => {
                spot * self.norm_cdf(d1) - strike * discount * self.norm_cdf(d2)
            }
            OptionType::Put => {
                strike * discount * self.norm_cdf(-d2) - spot * self.norm_cdf(-d1)
            }
        };

        Ok(price.max(0.0))
    }

    /// Calculate all Greeks for an option.
    pub fn calculate_greeks(
        &self,
        spot: f64,
        strike: f64,
        time_years: f64,
        volatility: f64,
        option_type: OptionType,
    ) -> Result<Greeks, OptionsError> {
        if spot <= 0.0 || strike <= 0.0 || time_years <= 0.0 || volatility <= 0.0 {
            return Err(OptionsError::InvalidInput(
                "All inputs must be positive".to_string(),
            ));
        }

        let d1 = self.d1(spot, strike, time_years, volatility);
        let d2 = self.d2(d1, volatility, time_years);
        let r = self.risk_free_rate;
        let discount = E.powf(-r * time_years);
        let sqrt_t = time_years.sqrt();

        // Delta
        let delta = match option_type {
            OptionType::Call => self.norm_cdf(d1),
            OptionType::Put => self.norm_cdf(d1) - 1.0,
        };

        // Gamma (same for calls and puts)
        let gamma = self.norm_pdf(d1) / (spot * volatility * sqrt_t);

        // Theta (per day)
        let theta = match option_type {
            OptionType::Call => {
                let term1 = -(spot * self.norm_pdf(d1) * volatility) / (2.0 * sqrt_t);
                let term2 = r * strike * discount * self.norm_cdf(d2);
                (term1 - term2) / 365.0
            }
            OptionType::Put => {
                let term1 = -(spot * self.norm_pdf(d1) * volatility) / (2.0 * sqrt_t);
                let term2 = r * strike * discount * self.norm_cdf(-d2);
                (term1 + term2) / 365.0
            }
        };

        // Vega (for 1% change in volatility)
        let vega = spot * sqrt_t * self.norm_pdf(d1) / 100.0;

        // Rho (for 1% change in interest rate)
        let rho = match option_type {
            OptionType::Call => strike * time_years * discount * self.norm_cdf(d2) / 100.0,
            OptionType::Put => -strike * time_years * discount * self.norm_cdf(-d2) / 100.0,
        };

        Ok(Greeks::new(delta, gamma, theta, vega, rho))
    }

    // ========== Implied Volatility ==========

    /// Calculate implied volatility using Newton-Raphson method.
    pub fn implied_volatility(
        &self,
        market_price: f64,
        spot: f64,
        strike: f64,
        time_years: f64,
        option_type: OptionType,
    ) -> Result<f64, OptionsError> {
        if market_price <= 0.0 || spot <= 0.0 || strike <= 0.0 || time_years <= 0.0 {
            return Err(OptionsError::InvalidInput(
                "All inputs must be positive".to_string(),
            ));
        }

        let max_iterations = 100;
        let tolerance = 1e-6;
        let mut vol = 0.2; // Initial guess: 20%

        for i in 0..max_iterations {
            let price = self.black_scholes_price(spot, strike, time_years, vol, option_type)?;
            let diff = price - market_price;

            if diff.abs() < tolerance {
                debug!(
                    "IV converged after {} iterations: {:.4}%",
                    i + 1,
                    vol * 100.0
                );
                return Ok(vol);
            }

            // Calculate vega for Newton-Raphson
            let d1 = self.d1(spot, strike, time_years, vol);
            let vega = spot * time_years.sqrt() * self.norm_pdf(d1);

            if vega.abs() < 1e-10 {
                // Vega too small, use bisection fallback
                break;
            }

            vol -= diff / vega;

            // Keep volatility in reasonable bounds
            vol = vol.max(0.001).min(5.0);
        }

        // Fallback to bisection method
        self.implied_volatility_bisection(market_price, spot, strike, time_years, option_type)
    }

    /// Calculate implied volatility using bisection method (fallback).
    fn implied_volatility_bisection(
        &self,
        market_price: f64,
        spot: f64,
        strike: f64,
        time_years: f64,
        option_type: OptionType,
    ) -> Result<f64, OptionsError> {
        let max_iterations = 200;
        let tolerance = 1e-6;
        let mut low = 0.001;
        let mut high = 5.0;

        for _ in 0..max_iterations {
            let mid = (low + high) / 2.0;
            let price = self.black_scholes_price(spot, strike, time_years, mid, option_type)?;
            let diff = price - market_price;

            if diff.abs() < tolerance {
                return Ok(mid);
            }

            if diff > 0.0 {
                high = mid;
            } else {
                low = mid;
            }
        }

        Err(OptionsError::IVConvergenceFailed(max_iterations))
    }

    // ========== Binomial Pricing (American Options) ==========

    /// Calculate option price using binomial tree (Cox-Ross-Rubinstein).
    /// Handles American-style options with early exercise.
    pub fn binomial_price(
        &self,
        spot: f64,
        strike: f64,
        time_years: f64,
        volatility: f64,
        option_type: OptionType,
        style: OptionStyle,
        steps: u32,
    ) -> Result<f64, OptionsError> {
        if spot <= 0.0 || strike <= 0.0 || time_years <= 0.0 || volatility <= 0.0 || steps == 0 {
            return Err(OptionsError::InvalidInput(
                "All inputs must be positive".to_string(),
            ));
        }

        let dt = time_years / steps as f64;
        let r = self.risk_free_rate;

        // Up and down factors
        let u = E.powf(volatility * dt.sqrt());
        let d = 1.0 / u;

        // Risk-neutral probability
        let p = (E.powf(r * dt) - d) / (u - d);
        let discount = E.powf(-r * dt);

        // Build price tree at expiration
        let n = steps as usize;
        let mut prices = vec![0.0; n + 1];

        for i in 0..=n {
            let spot_at_node = spot * u.powi(i as i32) * d.powi((n - i) as i32);
            prices[i] = match option_type {
                OptionType::Call => (spot_at_node - strike).max(0.0),
                OptionType::Put => (strike - spot_at_node).max(0.0),
            };
        }

        // Work backwards through tree
        for step in (0..n).rev() {
            for i in 0..=step {
                let spot_at_node = spot * u.powi(i as i32) * d.powi((step - i) as i32);
                let hold_value = discount * (p * prices[i + 1] + (1.0 - p) * prices[i]);

                prices[i] = match style {
                    OptionStyle::European => hold_value,
                    OptionStyle::American => {
                        let exercise_value = match option_type {
                            OptionType::Call => (spot_at_node - strike).max(0.0),
                            OptionType::Put => (strike - spot_at_node).max(0.0),
                        };
                        hold_value.max(exercise_value)
                    }
                };
            }
        }

        Ok(prices[0])
    }

    // ========== Option Pricing Wrapper ==========

    /// Calculate option price using appropriate method based on style.
    pub fn price_option(
        &self,
        spot: f64,
        strike: f64,
        time_years: f64,
        volatility: f64,
        option_type: OptionType,
        style: OptionStyle,
    ) -> Result<f64, OptionsError> {
        match style {
            OptionStyle::European => {
                self.black_scholes_price(spot, strike, time_years, volatility, option_type)
            }
            OptionStyle::American => {
                // Use binomial with 100 steps for American options
                self.binomial_price(spot, strike, time_years, volatility, option_type, style, 100)
            }
        }
    }

    /// Calculate full option analysis including price and Greeks.
    pub fn analyze_option(
        &self,
        spot: f64,
        strike: f64,
        time_years: f64,
        volatility: f64,
        option_type: OptionType,
        style: OptionStyle,
    ) -> Result<(f64, Greeks), OptionsError> {
        let price = self.price_option(spot, strike, time_years, volatility, option_type, style)?;
        let greeks = self.calculate_greeks(spot, strike, time_years, volatility, option_type)?;
        Ok((price, greeks))
    }

    /// Update an option position with current market data.
    pub fn update_position(
        &self,
        position: &mut OptionPosition,
        underlying_price: f64,
        market_premium: Option<f64>,
    ) -> Result<(), OptionsError> {
        let time_years = position.days_to_expiration() / 365.0;

        if time_years <= 0.0 {
            // Option expired
            position.current_premium = 0.0;
            position.greeks = Greeks::default();
            return Ok(());
        }

        // Use market premium if provided, otherwise calculate theoretical
        let premium = if let Some(mp) = market_premium {
            // Also update IV from market price
            if let Ok(iv) = self.implied_volatility(
                mp,
                underlying_price,
                position.strike,
                time_years,
                position.option_type,
            ) {
                position.current_iv = iv;
            }
            mp
        } else {
            self.price_option(
                underlying_price,
                position.strike,
                time_years,
                position.current_iv,
                position.option_type,
                position.style,
            )?
        };

        let greeks = self.calculate_greeks(
            underlying_price,
            position.strike,
            time_years,
            position.current_iv,
            position.option_type,
        )?;

        position.update(premium, underlying_price, greeks, position.current_iv);

        Ok(())
    }
}

/// Error function approximation for normal CDF.
fn erf(x: f64) -> f64 {
    // Horner form approximation
    let a1 = 0.254829592;
    let a2 = -0.284496736;
    let a3 = 1.421413741;
    let a4 = -1.453152027;
    let a5 = 1.061405429;
    let p = 0.3275911;

    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();

    let t = 1.0 / (1.0 + p * x);
    let y = 1.0 - (((((a5 * t + a4) * t) + a3) * t + a2) * t + a1) * t * E.powf(-x * x);

    sign * y
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_service() -> OptionsService {
        OptionsService::new(0.05) // 5% risk-free rate
    }

    #[test]
    fn test_black_scholes_call() {
        let service = setup_service();

        // ATM call: S=100, K=100, T=1yr, σ=20%
        let price = service
            .black_scholes_price(100.0, 100.0, 1.0, 0.20, OptionType::Call)
            .unwrap();

        // Expected around 10.45 for these parameters
        assert!((price - 10.45).abs() < 0.5);
    }

    #[test]
    fn test_black_scholes_put() {
        let service = setup_service();

        // ATM put: S=100, K=100, T=1yr, σ=20%
        let price = service
            .black_scholes_price(100.0, 100.0, 1.0, 0.20, OptionType::Put)
            .unwrap();

        // Expected around 5.57 for these parameters (put-call parity)
        assert!((price - 5.57).abs() < 0.5);
    }

    #[test]
    fn test_greeks_call() {
        let service = setup_service();

        let greeks = service
            .calculate_greeks(100.0, 100.0, 1.0, 0.20, OptionType::Call)
            .unwrap();

        // Delta for ATM call should be around 0.5-0.6
        assert!(greeks.delta > 0.5 && greeks.delta < 0.7);

        // Gamma should be positive
        assert!(greeks.gamma > 0.0);

        // Theta should be negative (time decay)
        assert!(greeks.theta < 0.0);

        // Vega should be positive
        assert!(greeks.vega > 0.0);
    }

    #[test]
    fn test_greeks_put() {
        let service = setup_service();

        let greeks = service
            .calculate_greeks(100.0, 100.0, 1.0, 0.20, OptionType::Put)
            .unwrap();

        // Delta for ATM put should be around -0.4 to -0.5
        assert!(greeks.delta < 0.0 && greeks.delta > -0.6);

        // Gamma should be same as call (same absolute value)
        assert!(greeks.gamma > 0.0);

        // Theta should be negative
        assert!(greeks.theta < 0.0);
    }

    #[test]
    fn test_implied_volatility() {
        let service = setup_service();

        // Get a price at known volatility
        let vol = 0.25;
        let price = service
            .black_scholes_price(100.0, 100.0, 1.0, vol, OptionType::Call)
            .unwrap();

        // Calculate IV from that price
        let calculated_iv = service
            .implied_volatility(price, 100.0, 100.0, 1.0, OptionType::Call)
            .unwrap();

        // Should recover the original volatility
        assert!((calculated_iv - vol).abs() < 0.001);
    }

    #[test]
    fn test_binomial_european() {
        let service = setup_service();

        // European option via binomial should match Black-Scholes
        let bs_price = service
            .black_scholes_price(100.0, 100.0, 1.0, 0.20, OptionType::Call)
            .unwrap();

        let binomial_price = service
            .binomial_price(
                100.0,
                100.0,
                1.0,
                0.20,
                OptionType::Call,
                OptionStyle::European,
                100,
            )
            .unwrap();

        // Should be very close (within 1%)
        assert!((binomial_price - bs_price).abs() / bs_price < 0.01);
    }

    #[test]
    fn test_binomial_american_put() {
        let service = setup_service();

        // American put should be >= European put (early exercise premium)
        let european_price = service
            .binomial_price(
                100.0,
                100.0,
                1.0,
                0.20,
                OptionType::Put,
                OptionStyle::European,
                100,
            )
            .unwrap();

        let american_price = service
            .binomial_price(
                100.0,
                100.0,
                1.0,
                0.20,
                OptionType::Put,
                OptionStyle::American,
                100,
            )
            .unwrap();

        assert!(american_price >= european_price);
    }

    #[test]
    fn test_deep_itm_call() {
        let service = setup_service();

        // Deep ITM call (S=150, K=100)
        let greeks = service
            .calculate_greeks(150.0, 100.0, 1.0, 0.20, OptionType::Call)
            .unwrap();

        // Delta should be close to 1
        assert!(greeks.delta > 0.9);
    }

    #[test]
    fn test_deep_otm_call() {
        let service = setup_service();

        // Deep OTM call (S=50, K=100)
        let greeks = service
            .calculate_greeks(50.0, 100.0, 1.0, 0.20, OptionType::Call)
            .unwrap();

        // Delta should be close to 0
        assert!(greeks.delta < 0.1);
    }

    #[test]
    fn test_invalid_inputs() {
        let service = setup_service();

        // Negative spot price should error
        assert!(service
            .black_scholes_price(-100.0, 100.0, 1.0, 0.20, OptionType::Call)
            .is_err());

        // Zero time should error
        assert!(service
            .black_scholes_price(100.0, 100.0, 0.0, 0.20, OptionType::Call)
            .is_err());

        // Negative volatility should error
        assert!(service
            .black_scholes_price(100.0, 100.0, 1.0, -0.20, OptionType::Call)
            .is_err());
    }
}
