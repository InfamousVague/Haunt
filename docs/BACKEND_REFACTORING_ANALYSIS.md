# Backend Refactoring Analysis for Granular Data Recording

## Current Architecture Overview

### 1. PriceCache (`src/services/price_cache.rs`)

**Purpose:** Multi-source price aggregation and real-time broadcasting

**Current Data Structures:**
```rust
pub struct PriceCache {
    prices: DashMap<String, SymbolPrice>,           // Current prices per symbol
    source_updates: DashMap<PriceSource, AtomicU64>, // Total updates per source
    symbol_source_updates: DashMap<String, DashMap<PriceSource, AtomicU64>>, // Per-symbol source updates
    recent_updates: Mutex<VecDeque<Instant>>,       // For TPS calculation (60s window)
    // ... other fields
}
```

**Strengths:**
- Efficient concurrent access with DashMap
- Good source tracking and online/offline status
- TPS calculation with sliding window

**Limitations:**
- No historical price tracking per symbol
- Cannot calculate percentage changes over arbitrary time windows
- Only tracks "current" state, not price history

### 2. ChartStore (`src/services/chart_store.rs`)

**Purpose:** Time-series OHLC data for charts and sparklines

**Current Data Structures:**
```rust
struct SymbolChartData {
    one_minute: TimeSeries,   // 1-min resolution, ~7 days retention
    five_minute: TimeSeries,  // 5-min resolution, ~30 days retention
    one_hour: TimeSeries,     // 1-hour resolution, ~1 year retention
}
```

**Strengths:**
- Multiple time resolutions
- OHLC data (open, high, low, close, volume)
- Redis persistence for sparklines
- Efficient VecDeque-based storage

**Limitations:**
- Designed for chart rendering, not real-time analytics
- No built-in methods for calculating price changes
- Limited API for querying historical data

---

## Requirements for Top Movers Feature

### Data Needs:
1. **Price change calculation** over multiple time windows:
   - 1 minute, 5 minutes, 15 minutes
   - 1 hour, 4 hours, 24 hours

2. **Efficient real-time updates:**
   - Recalculate movers on each price update
   - Broadcast top movers via WebSocket

3. **Sorting and filtering:**
   - Top gainers and losers
   - Configurable limit (top 10, 20, etc.)

### Performance Considerations:
- ~500+ symbols tracked
- ~50-100 updates/second (current TPS)
- Need sub-second recalculation

---

## Recommended Refactoring

### Option A: Extend ChartStore (Recommended)

**Rationale:** ChartStore already has time-series infrastructure. Add methods to calculate price changes.

**Changes Required:**

1. **Add price change calculation methods to ChartStore:**
```rust
impl ChartStore {
    /// Get price at a specific time ago
    pub fn get_price_at(&self, symbol: &str, seconds_ago: i64) -> Option<f64> {
        // Use 1-minute data for recent, 5-minute for older
    }

    /// Calculate percentage change over a time window
    pub fn get_price_change(&self, symbol: &str, seconds: i64) -> Option<f64> {
        let current = self.get_current_price(symbol)?;
        let past = self.get_price_at(symbol, seconds)?;
        Some(((current - past) / past) * 100.0)
    }

    /// Get top movers for a time window
    pub fn get_top_movers(&self, seconds: i64, limit: usize, direction: MoverDirection) -> Vec<Mover>;
}
```

2. **Add current price tracking:**
```rust
struct SymbolChartData {
    // ... existing fields
    current_price: Option<f64>,  // Latest price for quick access
    last_update: i64,            // Timestamp of last update
}
```

3. **Add Mover types:**
```rust
#[derive(Debug, Clone, Serialize)]
pub struct Mover {
    pub symbol: String,
    pub price: f64,
    pub change_percent: f64,
    pub volume_24h: Option<f64>,
}

pub enum MoverDirection {
    Gainers,
    Losers,
    Both,
}
```

**Pros:**
- Minimal new infrastructure
- Leverages existing time-series data
- Already has Redis persistence

**Cons:**
- ChartStore becomes more complex
- Coupling between chart and analytics concerns

---

### Option B: New MoversService

**Rationale:** Separation of concerns - dedicated service for analytics.

**New Service:**
```rust
pub struct MoversService {
    /// Price snapshots at fixed intervals
    snapshots: DashMap<String, PriceSnapshots>,
    /// Cached top movers by timeframe
    movers_cache: DashMap<TimeFrame, MoversCache>,
}

struct PriceSnapshots {
    /// Ring buffer of (timestamp, price) tuples
    history: VecDeque<(i64, f64)>,
    current_price: f64,
    volume_24h: Option<f64>,
}

struct MoversCache {
    gainers: Vec<Mover>,
    losers: Vec<Mover>,
    computed_at: Instant,
}
```

**Pros:**
- Clean separation of concerns
- Can optimize specifically for movers calculation
- Independent caching strategy

**Cons:**
- Duplicate price storage
- More complex system architecture
- Additional memory overhead

---

## Recommendation: Option A (Extend ChartStore)

ChartStore already has the time-series infrastructure needed. The changes are:

1. **Add current price field** to SymbolChartData
2. **Add helper methods** for price change calculation
3. **Add top movers API** with caching
4. **Add new API endpoint** `/api/market/movers`

### Implementation Plan:

#### Phase 1: Data Structure Updates
- Add `current_price` and `last_update` to SymbolChartData
- Update `add_price()` to maintain current price

#### Phase 2: Calculation Methods
- Implement `get_price_at()` using existing TimeSeries data
- Implement `get_price_change()` for arbitrary windows
- Add caching for expensive calculations

#### Phase 3: API Layer
- Add `get_top_movers()` method
- Create `/api/market/movers` endpoint
- Add WebSocket broadcast for movers updates

#### Phase 4: Optimization
- Add LRU cache for movers results
- Throttle recalculation (every 5-10 seconds)
- Consider pre-computed buckets for common timeframes

---

## Additional Metrics to Add

Beyond top movers, consider adding:

1. **Volume leaders** - Highest volume in last 24h
2. **Volatility index** - Price variance over time windows
3. **Source agreement** - How closely sources agree on price
4. **Update frequency** - Updates per symbol per minute

---

## Files to Modify

### Backend (Haunt):
1. `src/services/chart_store.rs` - Add price change methods
2. `src/types/market.rs` - Add Mover types
3. `src/api/market.rs` - Add movers endpoint
4. `src/services/mod.rs` - Export new types

### Frontend (Wraith):
1. `src/services/haunt.ts` - Add movers API client
2. `src/components/TopMoversCard.tsx` - New component

---

## Memory Considerations

Current estimated memory per symbol:
- PriceCache: ~500 bytes
- ChartStore: ~50KB (1-min: 10080 points, 5-min: 8640, 1-hour: 8760)

Adding current_price tracking: +16 bytes per symbol (negligible)

For 500 symbols: ~25MB total (acceptable)

---

## Conclusion

The existing ChartStore architecture is well-suited for the top movers feature. The refactoring is minimal and leverages existing time-series data. The main additions are:

1. Current price tracking in SymbolChartData
2. Helper methods for price change calculation
3. New API endpoint with caching

This approach maintains system simplicity while adding powerful new analytics capabilities.
