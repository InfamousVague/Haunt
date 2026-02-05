//! Liquidity Simulation Service
//!
//! Simulates realistic order execution using real order book data.
//! Provides:
//! - Order book walking for accurate VWAP calculation
//! - Partial fill simulation based on available liquidity
//! - Market impact modeling
//! - Volume-based limit order fill probability

#![allow(dead_code)]

use crate::types::{AggregatedOrderBook, AggregatedLevel, OrderSide};

/// Result of simulating a market order execution.
#[derive(Debug, Clone)]
pub struct MarketOrderSimulation {
    /// Volume-weighted average execution price
    pub vwap: f64,
    /// Total quantity that can be filled
    pub filled_quantity: f64,
    /// Quantity that couldn't be filled due to insufficient liquidity
    pub unfilled_quantity: f64,
    /// Whether the full order was filled
    pub fully_filled: bool,
    /// Slippage from mid price (absolute)
    pub slippage: f64,
    /// Slippage as percentage of mid price
    pub slippage_pct: f64,
    /// Market impact cost (notional slippage amount)
    pub impact_cost: f64,
    /// Number of price levels consumed
    pub levels_consumed: usize,
    /// Breakdown of fills at each level
    pub fills: Vec<LevelFill>,
}

/// A fill at a specific price level.
#[derive(Debug, Clone)]
pub struct LevelFill {
    /// Price at this level
    pub price: f64,
    /// Quantity filled at this level
    pub quantity: f64,
    /// Cumulative quantity filled up to and including this level
    pub cumulative_quantity: f64,
}

/// Result of simulating a limit order.
#[derive(Debug, Clone)]
pub struct LimitOrderSimulation {
    /// Whether the limit price is currently executable
    pub is_executable: bool,
    /// Quantity available at limit price or better
    pub available_quantity: f64,
    /// Whether full order can fill immediately
    pub can_fill_immediately: bool,
    /// Estimated fill probability (0.0 - 1.0) based on book depth
    pub fill_probability: f64,
    /// Estimated time to fill in seconds (based on volume)
    pub estimated_time_to_fill_secs: Option<f64>,
    /// Distance from best price (0 = at best price)
    pub distance_from_best: f64,
    /// Distance as percentage
    pub distance_from_best_pct: f64,
    /// Queue position estimate (how much volume ahead in queue)
    pub estimated_queue_depth: f64,
}

/// Configuration for liquidity simulation.
#[derive(Debug, Clone)]
pub struct LiquiditySimConfig {
    /// Multiplier for order book depth (simulates larger market)
    /// 1.0 = use real depth, 10.0 = simulate 10x the liquidity
    pub depth_multiplier: f64,
    /// Base market impact factor (% of book consumed -> % price impact)
    pub impact_factor: f64,
    /// Minimum fill rate per hour (% of order size) for limit orders
    pub min_fill_rate_per_hour: f64,
    /// Whether to allow partial fills for market orders
    pub allow_partial_fills: bool,
    /// Maximum slippage allowed (order fails if exceeded)
    pub max_slippage_pct: Option<f64>,
}

impl Default for LiquiditySimConfig {
    fn default() -> Self {
        Self {
            depth_multiplier: 1.0,
            impact_factor: 0.1,
            min_fill_rate_per_hour: 0.05, // 5% of order per hour minimum
            allow_partial_fills: true,
            max_slippage_pct: None,
        }
    }
}

/// Service for simulating realistic order execution.
pub struct LiquiditySimulator {
    config: LiquiditySimConfig,
}

impl LiquiditySimulator {
    /// Create a new liquidity simulator.
    pub fn new(config: LiquiditySimConfig) -> Self {
        Self { config }
    }

    /// Simulate a market order execution by walking the order book.
    ///
    /// For a buy order, walks the asks (ascending price).
    /// For a sell order, walks the bids (descending price).
    pub fn simulate_market_order(
        &self,
        order_book: &AggregatedOrderBook,
        side: OrderSide,
        quantity: f64,
    ) -> MarketOrderSimulation {
        let levels = match side {
            OrderSide::Buy => &order_book.asks,
            OrderSide::Sell => &order_book.bids,
        };

        let mid_price = order_book.mid_price;
        let mut remaining = quantity;
        let mut total_cost = 0.0;
        let mut filled = 0.0;
        let mut fills = Vec::new();
        let mut levels_consumed = 0;

        for level in levels {
            if remaining <= 0.0 {
                break;
            }

            // Apply depth multiplier to simulate more/less liquidity
            let available = level.total_quantity * self.config.depth_multiplier;
            let fill_qty = remaining.min(available);

            filled += fill_qty;
            total_cost += fill_qty * level.price;
            remaining -= fill_qty;
            levels_consumed += 1;

            fills.push(LevelFill {
                price: level.price,
                quantity: fill_qty,
                cumulative_quantity: filled,
            });
        }

        let vwap = if filled > 0.0 {
            total_cost / filled
        } else {
            mid_price
        };

        let slippage = match side {
            OrderSide::Buy => vwap - mid_price,
            OrderSide::Sell => mid_price - vwap,
        };

        let slippage_pct = if mid_price > 0.0 {
            (slippage / mid_price) * 100.0
        } else {
            0.0
        };

        let impact_cost = slippage.abs() * filled;

        MarketOrderSimulation {
            vwap,
            filled_quantity: filled,
            unfilled_quantity: remaining.max(0.0),
            fully_filled: remaining <= 0.0,
            slippage,
            slippage_pct,
            impact_cost,
            levels_consumed,
            fills,
        }
    }

    /// Simulate a limit order to determine fill probability and timing.
    pub fn simulate_limit_order(
        &self,
        order_book: &AggregatedOrderBook,
        side: OrderSide,
        quantity: f64,
        limit_price: f64,
        volume_24h: Option<f64>,
    ) -> LimitOrderSimulation {
        let (levels, best_price, is_executable) = match side {
            OrderSide::Buy => {
                // Buy limit: executable if limit_price >= best_ask
                let executable = limit_price >= order_book.best_ask;
                (&order_book.asks, order_book.best_ask, executable)
            }
            OrderSide::Sell => {
                // Sell limit: executable if limit_price <= best_bid
                let executable = limit_price <= order_book.best_bid;
                (&order_book.bids, order_book.best_bid, executable)
            }
        };

        // Calculate available quantity at limit price or better
        let available_quantity: f64 = levels
            .iter()
            .filter(|l| match side {
                OrderSide::Buy => l.price <= limit_price,
                OrderSide::Sell => l.price >= limit_price,
            })
            .map(|l| l.total_quantity * self.config.depth_multiplier)
            .sum();

        let can_fill_immediately = is_executable && available_quantity >= quantity;

        // Distance from best price
        let distance = match side {
            OrderSide::Buy => (order_book.best_ask - limit_price).max(0.0),
            OrderSide::Sell => (limit_price - order_book.best_bid).max(0.0),
        };
        let distance_pct = if best_price > 0.0 {
            (distance / best_price) * 100.0
        } else {
            0.0
        };

        // Estimate queue depth (volume ahead of us at our price level)
        let queue_depth = self.estimate_queue_depth(levels, side, limit_price);

        // Calculate fill probability based on position relative to spread
        let fill_probability = self.calculate_fill_probability(
            order_book,
            side,
            limit_price,
            is_executable,
        );

        // Estimate time to fill based on 24h volume
        let estimated_time = volume_24h.and_then(|vol| {
            if vol > 0.0 && !is_executable {
                // Estimate based on historical volume and queue position
                let hourly_volume = vol / 24.0;
                let effective_queue = queue_depth + quantity;

                // Time = queue depth / (hourly volume * fill rate at this distance)
                let fill_rate = self.config.min_fill_rate_per_hour +
                    (1.0 - distance_pct / 10.0).max(0.0) * 0.5;

                Some(effective_queue / (hourly_volume * fill_rate) * 3600.0)
            } else if is_executable {
                Some(0.0) // Immediate fill
            } else {
                None
            }
        });

        LimitOrderSimulation {
            is_executable,
            available_quantity,
            can_fill_immediately,
            fill_probability,
            estimated_time_to_fill_secs: estimated_time,
            distance_from_best: distance,
            distance_from_best_pct: distance_pct,
            estimated_queue_depth: queue_depth,
        }
    }

    /// Calculate the execution price for a market order with realistic slippage.
    /// Returns (execution_price, slippage_amount, filled_quantity).
    pub fn calculate_execution_price(
        &self,
        order_book: &AggregatedOrderBook,
        side: OrderSide,
        quantity: f64,
    ) -> (f64, f64, f64) {
        let sim = self.simulate_market_order(order_book, side, quantity);

        // Check max slippage if configured
        if let Some(max_slip) = self.config.max_slippage_pct {
            if sim.slippage_pct.abs() > max_slip {
                // Return mid price with max allowed slippage
                let capped_price = match side {
                    OrderSide::Buy => order_book.mid_price * (1.0 + max_slip / 100.0),
                    OrderSide::Sell => order_book.mid_price * (1.0 - max_slip / 100.0),
                };
                let capped_slippage = order_book.mid_price * max_slip / 100.0;
                return (capped_price, capped_slippage, sim.filled_quantity);
            }
        }

        (sim.vwap, sim.slippage.abs(), sim.filled_quantity)
    }

    /// Simulate partial fill for IOC/FOK orders based on available liquidity.
    /// Returns quantity that would fill immediately at the given price or better.
    pub fn available_fill_quantity(
        &self,
        order_book: &AggregatedOrderBook,
        side: OrderSide,
        max_price: Option<f64>, // None for market orders
    ) -> f64 {
        let levels = match side {
            OrderSide::Buy => &order_book.asks,
            OrderSide::Sell => &order_book.bids,
        };

        levels
            .iter()
            .filter(|l| {
                if let Some(max) = max_price {
                    match side {
                        OrderSide::Buy => l.price <= max,
                        OrderSide::Sell => l.price >= max,
                    }
                } else {
                    true
                }
            })
            .map(|l| l.total_quantity * self.config.depth_multiplier)
            .sum()
    }

    /// Check if a limit order would have been filled based on price movement.
    /// Used for backtesting with historical candles.
    pub fn would_limit_fill(
        &self,
        side: OrderSide,
        limit_price: f64,
        candle_high: f64,
        candle_low: f64,
    ) -> bool {
        match side {
            // Buy limit fills if price dropped to or below limit
            OrderSide::Buy => candle_low <= limit_price,
            // Sell limit fills if price rose to or above limit
            OrderSide::Sell => candle_high >= limit_price,
        }
    }

    /// Calculate how much of a limit order would fill based on volume.
    /// Used for more realistic partial fill simulation in backtesting.
    pub fn estimate_limit_fill_quantity(
        &self,
        side: OrderSide,
        order_quantity: f64,
        limit_price: f64,
        candle_high: f64,
        candle_low: f64,
        candle_volume: f64,
    ) -> f64 {
        if !self.would_limit_fill(side, limit_price, candle_high, candle_low) {
            return 0.0;
        }

        // Estimate fill based on how deep into the candle the limit price is
        let candle_range = candle_high - candle_low;
        if candle_range <= 0.0 {
            // Flat candle, assume full fill if price touched
            return order_quantity;
        }

        let depth_ratio = match side {
            OrderSide::Buy => {
                // How far below the high did the price need to go?
                (candle_high - limit_price) / candle_range
            }
            OrderSide::Sell => {
                // How far above the low did the price need to go?
                (limit_price - candle_low) / candle_range
            }
        };

        // Less volume trades at extreme prices
        // Use exponential decay: more volume near open/close, less at extremes
        let volume_at_level = candle_volume * (1.0 - depth_ratio).powf(2.0);

        // Can only fill up to the available volume at this level
        order_quantity.min(volume_at_level * 0.1) // Assume 10% of volume at any price level
    }

    /// Estimate queue depth at a price level.
    fn estimate_queue_depth(
        &self,
        levels: &[AggregatedLevel],
        side: OrderSide,
        limit_price: f64,
    ) -> f64 {
        // Find levels at or better than our limit price
        levels
            .iter()
            .filter(|l| match side {
                OrderSide::Buy => l.price < limit_price,
                OrderSide::Sell => l.price > limit_price,
            })
            .map(|l| l.total_quantity * self.config.depth_multiplier)
            .sum()
    }

    /// Calculate fill probability for a limit order.
    fn calculate_fill_probability(
        &self,
        order_book: &AggregatedOrderBook,
        side: OrderSide,
        limit_price: f64,
        is_executable: bool,
    ) -> f64 {
        if is_executable {
            return 1.0;
        }

        // Calculate distance as percentage of spread
        let spread = order_book.spread;
        if spread <= 0.0 {
            return 0.5;
        }

        let distance = match side {
            OrderSide::Buy => order_book.best_ask - limit_price,
            OrderSide::Sell => limit_price - order_book.best_bid,
        };

        // Probability decreases with distance
        // At best price: ~95%
        // At 1 spread away: ~50%
        // At 5 spreads away: ~5%
        let spreads_away = distance / spread;
        (0.95 * (-0.7 * spreads_away).exp()).max(0.01)
    }
}

impl Default for LiquiditySimulator {
    fn default() -> Self {
        Self::new(LiquiditySimConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::AggregatedOrderBook;
    use std::collections::HashMap;

    fn create_test_order_book() -> AggregatedOrderBook {
        let bids = vec![
            AggregatedLevel {
                price: 50000.0,
                total_quantity: 10.0,
                exchanges: HashMap::new(),
            },
            AggregatedLevel {
                price: 49990.0,
                total_quantity: 20.0,
                exchanges: HashMap::new(),
            },
            AggregatedLevel {
                price: 49980.0,
                total_quantity: 30.0,
                exchanges: HashMap::new(),
            },
        ];

        let asks = vec![
            AggregatedLevel {
                price: 50010.0,
                total_quantity: 10.0,
                exchanges: HashMap::new(),
            },
            AggregatedLevel {
                price: 50020.0,
                total_quantity: 20.0,
                exchanges: HashMap::new(),
            },
            AggregatedLevel {
                price: 50030.0,
                total_quantity: 30.0,
                exchanges: HashMap::new(),
            },
        ];

        AggregatedOrderBook {
            symbol: "BTC".to_string(),
            bids,
            asks,
            bid_total: 60.0,
            ask_total: 60.0,
            imbalance: 0.0,
            best_bid: 50000.0,
            best_ask: 50010.0,
            spread: 10.0,
            spread_pct: 0.02,
            mid_price: 50005.0,
            exchange_count: 1,
            exchanges: vec!["test".to_string()],
            timestamp: 0,
        }
    }

    #[test]
    fn test_market_buy_small_order() {
        let sim = LiquiditySimulator::default();
        let book = create_test_order_book();

        // Small order should fill entirely at best ask
        let result = sim.simulate_market_order(&book, OrderSide::Buy, 5.0);

        assert!(result.fully_filled);
        assert_eq!(result.filled_quantity, 5.0);
        assert_eq!(result.vwap, 50010.0); // Best ask
        assert_eq!(result.levels_consumed, 1);
    }

    #[test]
    fn test_market_buy_walks_book() {
        let sim = LiquiditySimulator::default();
        let book = create_test_order_book();

        // Order larger than first level
        let result = sim.simulate_market_order(&book, OrderSide::Buy, 15.0);

        assert!(result.fully_filled);
        assert_eq!(result.filled_quantity, 15.0);
        assert_eq!(result.levels_consumed, 2);

        // VWAP = (10 * 50010 + 5 * 50020) / 15
        let expected_vwap = (10.0 * 50010.0 + 5.0 * 50020.0) / 15.0;
        assert!((result.vwap - expected_vwap).abs() < 0.01);
    }

    #[test]
    fn test_market_sell_walks_book() {
        let sim = LiquiditySimulator::default();
        let book = create_test_order_book();

        let result = sim.simulate_market_order(&book, OrderSide::Sell, 15.0);

        assert!(result.fully_filled);
        assert_eq!(result.levels_consumed, 2);

        // VWAP = (10 * 50000 + 5 * 49990) / 15
        let expected_vwap = (10.0 * 50000.0 + 5.0 * 49990.0) / 15.0;
        assert!((result.vwap - expected_vwap).abs() < 0.01);
    }

    #[test]
    fn test_market_order_insufficient_liquidity() {
        let sim = LiquiditySimulator::default();
        let book = create_test_order_book();

        // Order larger than total book
        let result = sim.simulate_market_order(&book, OrderSide::Buy, 100.0);

        assert!(!result.fully_filled);
        assert_eq!(result.filled_quantity, 60.0); // Total ask volume
        assert_eq!(result.unfilled_quantity, 40.0);
    }

    #[test]
    fn test_limit_order_executable() {
        let sim = LiquiditySimulator::default();
        let book = create_test_order_book();

        // Buy at or above best ask = executable
        let result = sim.simulate_limit_order(&book, OrderSide::Buy, 5.0, 50010.0, None);

        assert!(result.is_executable);
        assert!(result.can_fill_immediately);
        assert_eq!(result.fill_probability, 1.0);
    }

    #[test]
    fn test_limit_order_below_market() {
        let sim = LiquiditySimulator::default();
        let book = create_test_order_book();

        // Buy limit below best ask
        let result = sim.simulate_limit_order(&book, OrderSide::Buy, 5.0, 50000.0, None);

        assert!(!result.is_executable);
        assert!(!result.can_fill_immediately);
        assert!(result.fill_probability < 1.0);
        assert_eq!(result.distance_from_best, 10.0);
    }

    #[test]
    fn test_depth_multiplier() {
        let config = LiquiditySimConfig {
            depth_multiplier: 10.0,
            ..Default::default()
        };
        let sim = LiquiditySimulator::new(config);
        let book = create_test_order_book();

        // With 10x multiplier, 50 units should fill at first level only
        let result = sim.simulate_market_order(&book, OrderSide::Buy, 50.0);

        assert!(result.fully_filled);
        assert_eq!(result.levels_consumed, 1); // 10 * 10 = 100 available at first level
    }

    #[test]
    fn test_would_limit_fill() {
        let sim = LiquiditySimulator::default();

        // Buy limit at 50000, candle low was 49900
        assert!(sim.would_limit_fill(OrderSide::Buy, 50000.0, 50100.0, 49900.0));

        // Buy limit at 50000, candle low was 50100 (never touched)
        assert!(!sim.would_limit_fill(OrderSide::Buy, 50000.0, 50200.0, 50100.0));

        // Sell limit at 50000, candle high was 50100
        assert!(sim.would_limit_fill(OrderSide::Sell, 50000.0, 50100.0, 49900.0));

        // Sell limit at 50000, candle high was 49900 (never touched)
        assert!(!sim.would_limit_fill(OrderSide::Sell, 50000.0, 49900.0, 49800.0));
    }

    #[test]
    fn test_calculate_execution_price() {
        let sim = LiquiditySimulator::default();
        let book = create_test_order_book();

        let (price, slippage, filled) = sim.calculate_execution_price(&book, OrderSide::Buy, 5.0);

        assert_eq!(filled, 5.0);
        assert_eq!(price, 50010.0);
        assert!(slippage > 0.0); // Should have some slippage from mid
    }
}
