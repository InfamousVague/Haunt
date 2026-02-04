# Frontend Wiring Guide: Trading, Bots & Leaderboard

> **For the next agent:** This document contains comprehensive instructions for wiring up the new trading, bot, and leaderboard functionality to the Wraith frontend. The backend is complete - all APIs are live and tested.

---

## Wraith Frontend Reference

The frontend is located at `/Users/infamousvague/Development/Wraith` and uses:

- **Stack**: React 19 + TypeScript + Vite + TailwindCSS
- **State**: Zustand stores + React Query
- **UI**: shadcn/ui components + Radix primitives
- **API Client**: `src/services/hauntClient.ts` - already has trading methods!
- **WebSocket**: `src/contexts/HauntSocketProvider.tsx` - already connected!

### Existing Components You Can Reuse

| Component | Location | Use For |
|-----------|----------|---------|
| `Leaderboard` page | `src/pages/Leaderboard.tsx` | Already exists! Has TraderRow, fetches leaderboard |
| `TraderRow` | `src/components/leaderboard/TraderRow.tsx` | Leaderboard row component |
| `TradeView` | `src/pages/TradeView.tsx` | Trading page with chart |
| `TradeSandbox` | `src/pages/TradeSandbox.tsx` | Paper trading sandbox |
| `OrderForm` | `src/components/trading/OrderForm.tsx` | Order entry form |
| `PositionsTable` | `src/components/trading/PositionsTable.tsx` | Open positions display |
| `OrdersTable` | `src/components/trading/OrdersTable.tsx` | Orders display |
| `Portfolio` page | `src/pages/Portfolio.tsx` | User portfolio page |
| `RecommendationCard` | `src/components/recommendations/` | Signal/recommendation display |
| `SignalSummaryCard` | `src/components/signals/` | Trading signal cards |

### HauntClient API Service

`src/services/hauntClient.ts` already has these trading methods:

```typescript
// Portfolio
getPortfolios(userId: string): Promise<Portfolio[]>
createPortfolio(data: CreatePortfolioRequest): Promise<Portfolio>
getPortfolioSummary(portfolioId: string): Promise<PortfolioSummary>

// Orders
placeOrder(data: PlaceOrderRequest): Promise<Order>
getOrders(portfolioId: string): Promise<Order[]>
cancelOrder(orderId: string): Promise<void>

// Positions
getPositions(portfolioId: string): Promise<Position[]>
modifyPosition(positionId: string, data: ModifyPositionRequest): Promise<Position>
closePosition(positionId: string, price: number): Promise<Trade>

// Trades
getTrades(portfolioId: string): Promise<Trade[]>

// Leaderboard
getLeaderboard(limit?: number): Promise<LeaderboardEntry[]>
```

### HauntSocketProvider

`src/contexts/HauntSocketProvider.tsx` provides WebSocket connection:

```tsx
// Usage in components
const { socket, connected, subscribe, unsubscribe } = useHauntSocket();

// Subscribe to trading channel
useEffect(() => {
  subscribe('trading', { portfolioId });
  return () => unsubscribe('trading');
}, [portfolioId]);

// Listen for events
useEffect(() => {
  if (!socket) return;
  const handler = (event: MessageEvent) => {
    const msg = JSON.parse(event.data);
    if (msg.type === 'positionUpdate') {
      // Handle position update
    }
  };
  socket.addEventListener('message', handler);
  return () => socket.removeEventListener('message', handler);
}, [socket]);
```

### Existing Routes

Check `src/App.tsx` for route definitions:
- `/leaderboard` - Leaderboard page exists
- `/trade` - TradeView page
- `/trade-sandbox` - TradeSandbox page
- `/portfolio` - Portfolio page

---

## Quick Context

We've implemented a paper trading system with AI-powered trading bots. Here's what's new:

1. **Trading System** - Full paper trading with portfolios, orders, positions
2. **AI Bots** - Grandma bot (conservative SMA crossover trader) is live
3. **Leaderboard** - Rankings by portfolio return percentage
4. **Bot Following** - Users can follow bots and copy trades

---

## API Base URL

```
Base: /api
WebSocket: /ws
```

## ⚠️ IMPORTANT: Correct Endpoint Paths

The frontend currently uses incorrect paths. Here are the **correct** backend endpoints:

| Frontend Uses (WRONG) | Backend Path (CORRECT) | Description |
|-----------------------|------------------------|-------------|
| `/api/leaderboard` | `/api/trading/leaderboard` | Get leaderboard rankings |
| `/api/portfolio` | `/api/trading/portfolios` | List portfolios (needs `?user_id=xxx`) |
| `/api/positions` | `/api/trading/positions` | List positions (needs `?portfolio_id=xxx`) |
| `/api/orders` | `/api/trading/orders` | List orders (needs `?portfolio_id=xxx`) |
| `/api/trades` | `/api/trading/trades` | List trades (needs `?portfolio_id=xxx`) |

### Frontend Fix Required

Update `src/services/hauntClient.ts` to use these correct paths:

```typescript
// WRONG:
getLeaderboard() { return this.fetch('/api/leaderboard'); }

// CORRECT:
getLeaderboard(limit = 100) { return this.fetch(`/api/trading/leaderboard?limit=${limit}`); }

// WRONG:
getPortfolios() { return this.fetch('/api/portfolio'); }

// CORRECT:
getPortfolios(userId: string) { return this.fetch(`/api/trading/portfolios?user_id=${userId}`); }

// WRONG:
getPositions() { return this.fetch('/api/positions'); }

// CORRECT:
getPositions(portfolioId: string) { return this.fetch(`/api/trading/positions?portfolio_id=${portfolioId}`); }

// WRONG:
getOrders() { return this.fetch('/api/orders'); }

// CORRECT:
getOrders(portfolioId: string) { return this.fetch(`/api/trading/orders?portfolio_id=${portfolioId}`); }

// WRONG:
getTrades() { return this.fetch('/api/trades'); }

// CORRECT:
getTrades(portfolioId: string, limit = 100) { return this.fetch(`/api/trading/trades?portfolio_id=${portfolioId}&limit=${limit}`); }
```

---

## Phase 1: Leaderboard Page

### Prompt
> "We have a new leaderboard system that ranks all portfolios by return percentage. The Leaderboard page already exists but needs to be enhanced to show bots with special badges and include new metrics like win rate."

### Existing Components (in Wraith)
- ✅ `src/pages/Leaderboard.tsx` - Page exists, update to use new LeaderboardEntry fields
- ✅ `src/components/leaderboard/TraderRow.tsx` - Row component exists, add bot badge
- ✅ `HauntClient.getLeaderboard()` - API method already exists!

### Tasks

#### Step 1: Verify API Connection
- [ ] Open `src/services/hauntClient.ts`
- [ ] Find `getLeaderboard()` method
- [ ] Verify it calls `GET /api/trading/leaderboard`
- [ ] Check return type matches `LeaderboardEntry[]`
- [ ] Test the API call in browser console: `await hauntClient.getLeaderboard(10)`

#### Step 2: Update Leaderboard Page (`src/pages/Leaderboard.tsx`)
- [ ] Import `HauntClient` if not already
- [ ] Replace any mock/dummy data with `HauntClient.getLeaderboard()`
- [ ] Use React Query or useEffect to fetch data on mount
- [ ] Add loading state while fetching
- [ ] Add error state for failed fetches
- [ ] Wire "limit" query param (default 100)

#### Step 3: Update LeaderboardEntry Type (`src/types/` or inline)
- [ ] Add or verify `LeaderboardEntry` interface matches backend:
  ```typescript
  interface LeaderboardEntry {
    portfolioId: string;
    name: string;
    userId: string;
    totalValue: number;
    startingBalance: number;
    realizedPnl: number;
    unrealizedPnl: number;
    totalReturnPct: number;
    totalTrades: number;
    winningTrades: number;
    winRate: number;
  }
  ```

#### Step 4: Update TraderRow Component (`src/components/leaderboard/TraderRow.tsx`)
- [ ] Add bot detection: `const isBot = entry.userId.startsWith('bot_')`
- [ ] Conditionally render bot badge (robot icon) when `isBot` is true
- [ ] Add `winRate` column: `{(entry.winRate * 100).toFixed(1)}%`
- [ ] Add `totalTrades` column
- [ ] Make row clickable → navigate to `/portfolio/${entry.portfolioId}`

#### Step 5: Add Bot Badge Component
- [ ] Create `BotBadge` component or inline:
  ```tsx
  {isBot && (
    <span className="ml-2 inline-flex items-center gap-1 text-xs text-blue-500">
      <Bot className="h-3 w-3" /> Bot
    </span>
  )}
  ```
- [ ] Import Bot icon from lucide-react

#### Step 6: Add Rank Badges (Optional)
- [ ] Add gold/silver/bronze badge for rank 1/2/3
- [ ] Use Trophy or Medal icons from lucide-react

- [x] **Backend leaderboard API endpoint** (DONE)
  - [x] `GET /api/trading/leaderboard?limit=100` endpoint added
  - [x] Returns `LeaderboardEntry[]` sorted by `totalReturnPct` descending

### API Reference

**GET /api/trading/leaderboard?limit=100**
```typescript
// Response wrapper
interface ApiResponse<T> {
  data: T;
}

// Response
interface LeaderboardEntry {
  portfolioId: string;
  name: string;
  userId: string;
  totalValue: number;
  startingBalance: number;
  realizedPnl: number;
  unrealizedPnl: number;
  totalReturnPct: number;
  totalTrades: number;
  winningTrades: number;
  winRate: number;
}

// Example
GET /api/trading/leaderboard?limit=50
// Returns: { data: LeaderboardEntry[] }
```

### UI Components
- [x] `LeaderboardTable` - Already exists in `src/pages/Leaderboard.tsx`
- [x] `TraderRow` - Already exists in `src/components/leaderboard/TraderRow.tsx`
- [ ] `BotBadge` - Add to TraderRow for AI bot detection
- [ ] `RankBadge` - Add Gold/Silver/Bronze for top 3

---

## Phase 2: AI Bots Page

### Prompt
> "We now have AI trading bots! Grandma is our first bot - she's a conservative trader using SMA crossover strategy. I think it'd be awesome to have a Bots page where users can see all active bots, their strategies, performance, and recent trades. Users should be able to follow bots to get notifications when they trade."

### Existing Components to Reference
- `src/components/recommendations/RecommendationCard.tsx` - Similar card layout for bot display
- `src/components/signals/SignalSummaryCard.tsx` - Stats display pattern
- `src/components/trading/` - Trading components for trade history display

### Tasks

#### Step 1: Add Bot API Methods to HauntClient (`src/services/hauntClient.ts`)
- [ ] Add `getBots()` method:
  ```typescript
  async getBots(): Promise<BotStatus[]> {
    const response = await this.fetch('/api/bots');
    return response.bots;
  }
  ```
- [ ] Add `getBot(botId: string)` method
- [ ] Add `getBotPerformance(botId: string)` method
- [ ] Add `getBotTrades(botId: string)` method
- [ ] Add `followBot(botId: string, userId: string)` method
- [ ] Add `unfollowBot(botId: string, userId: string)` method

#### Step 2: Add BotStatus Types (`src/types/` or in hauntClient)
- [ ] Add interfaces:
  ```typescript
  interface BotStatus {
    id: string;
    name: string;
    personality: "grandma" | "crypto_bro" | "quant";
    running: boolean;
    portfolioId: string | null;
    totalTrades: number;
    winningTrades: number;
    totalPnl: number;
    portfolioValue: number;
    lastDecisionAt: number | null;
    lastError: string | null;
    assetClasses: ("crypto_spot" | "stock" | "forex")[];
  }

  interface BotPerformance {
    botId: string;
    name: string;
    personality: string;
    totalTrades: number;
    winningTrades: number;
    winRate: number;
    totalPnl: number;
    portfolioValue: number;
    returnPct: number;
    sharpeRatio: number | null;
    maxDrawdown: number | null;
  }
  ```

#### Step 3: Create Bots List Page (`src/pages/BotsPage.tsx`)
- [ ] Create new file `src/pages/BotsPage.tsx`
- [ ] Fetch bots: `useQuery(['bots'], () => HauntClient.getBots())`
- [ ] Add loading skeleton while fetching
- [ ] Add error state with retry button
- [ ] Map bots to `BotCard` components
- [ ] Add page header: "AI Trading Bots"

#### Step 4: Create BotCard Component (`src/components/bots/BotCard.tsx`)
- [ ] Create `src/components/bots/BotCard.tsx`
- [ ] Display: Avatar/Icon, Name, Personality Badge, Status (Running/Stopped)
- [ ] Show stats: Win Rate, Total PnL, Portfolio Value
- [ ] Add personality-specific colors:
  - Grandma: warm/cozy (amber/orange)
  - Crypto Bro: neon/energetic (green/cyan)
  - Quant: cool/calculated (blue/purple)
- [ ] Make entire card clickable → `/bots/${bot.id}`

#### Step 5: Create Bot Detail Page (`src/pages/BotDetailPage.tsx`)
- [ ] Create new file `src/pages/BotDetailPage.tsx`
- [ ] Get `botId` from route params: `const { botId } = useParams()`
- [ ] Fetch bot details: `useQuery(['bot', botId], () => HauntClient.getBot(botId))`
- [ ] Fetch performance: `useQuery(['botPerformance', botId], () => HauntClient.getBotPerformance(botId))`
- [ ] Fetch trades: `useQuery(['botTrades', botId], () => HauntClient.getBotTrades(botId))`

#### Step 6: Create Bot Detail Components
- [ ] Create `BotDetailHeader.tsx` - Name, avatar, personality, status badge
- [ ] Create `BotPerformanceStats.tsx` - Grid of stats cards:
  - Win Rate, Total PnL, Return %, Portfolio Value
  - Sharpe Ratio, Max Drawdown (if available)
- [ ] Create `BotTradeHistory.tsx` - Table of recent trades (reuse TradesTable pattern)
- [ ] Create `BotStrategyCard.tsx` - Display strategy description from BOT_DESCRIPTIONS

#### Step 7: Add Follow/Unfollow Feature
- [ ] Create `FollowBotButton.tsx` component
- [ ] Check if user follows bot: `useQuery(['following', botId], () => HauntClient.isFollowingBot(botId))`
- [ ] Toggle handler:
  ```tsx
  const handleToggle = async () => {
    if (isFollowing) {
      await HauntClient.unfollowBot(botId, userId);
    } else {
      await HauntClient.followBot(botId, userId);
    }
    // Invalidate following query
  };
  ```
- [ ] Show loading state during toggle
- [ ] Show success toast: "Now following Grandma!" / "Unfollowed Grandma"

#### Step 8: Add Routes (`src/App.tsx`)
- [ ] Import BotsPage and BotDetailPage
- [ ] Add routes:
  ```tsx
  <Route path="/bots" element={<BotsPage />} />
  <Route path="/bots/:botId" element={<BotDetailPage />} />
  ```

### API Reference

**GET /api/bots** - List all bots
```typescript
interface BotsListResponse {
  bots: BotStatus[];
  total: number;
}

interface BotStatus {
  id: string;              // "grandma"
  name: string;            // "Grandma"
  personality: "grandma" | "crypto_bro" | "quant";
  running: boolean;
  portfolioId: string | null;
  totalTrades: number;
  winningTrades: number;
  totalPnl: number;
  portfolioValue: number;
  lastDecisionAt: number | null;  // Unix timestamp
  lastError: string | null;
  assetClasses: ("crypto_spot" | "stock" | "forex")[];
}
```

**GET /api/bots/:botId** - Get single bot
```typescript
interface BotResponse {
  bot: BotStatus;
}
```

**GET /api/bots/:botId/performance** - Bot metrics
```typescript
interface BotPerformance {
  botId: string;
  name: string;
  personality: string;
  totalTrades: number;
  winningTrades: number;
  winRate: number;          // 0.0 - 1.0
  totalPnl: number;
  portfolioValue: number;
  returnPct: number;        // e.g., 5.5 for 5.5%
  sharpeRatio: number | null;
  maxDrawdown: number | null;
}
```

**GET /api/bots/:botId/trades** - Bot's trade history
```typescript
interface Trade {
  id: string;
  orderId: string;
  portfolioId: string;
  symbol: string;
  assetClass: string;
  side: "buy" | "sell";
  quantity: number;
  price: number;
  fee: number;
  slippage: number;
  executedAt: number;  // Unix timestamp ms
}
```

**POST /api/bots/:botId/follow** - Follow a bot
```typescript
// Request
interface FollowBotRequest {
  userId: string;
  autoCopy?: boolean;  // If true, auto-copy trades to user's portfolio
}

// Response
interface FollowResponse {
  success: boolean;
  message: string;
}
```

**POST /api/bots/:botId/unfollow** - Unfollow a bot
```typescript
// Same request/response as follow
```

### UI Components Needed
- [ ] `BotCard` - Summary card for bot list
- [ ] `BotDetailHeader` - Bot name, personality, status
- [ ] `BotPerformanceStats` - Key metrics display
- [ ] `BotTradeHistory` - Trade table with filters
- [ ] `FollowBotButton` - Toggle follow state
- [ ] `PersonalityBadge` - Visual for Grandma/CryptoBro/Quant

### Bot Personality Descriptions
```typescript
const BOT_DESCRIPTIONS = {
  grandma: {
    name: "Grandma",
    tagline: "Slow and steady wins the race",
    description: "Conservative trader using simple moving averages. Trades crypto, stocks, and forex with a focus on long-term trends.",
    strategy: "SMA 50/200 crossover with RSI confirmation",
    riskLevel: "Low",
    tradingStyle: "Swing trading, max 1 trade/day"
  },
  crypto_bro: {
    name: "Crypto Bro",
    tagline: "WAGMI! Diamond hands!",
    description: "Aggressive momentum chaser. High risk, high reward. Prefers crypto and meme stocks.",
    strategy: "RSI momentum + MACD + FOMO triggers",
    riskLevel: "High",
    tradingStyle: "Day trading, very active"
  },
  quant: {
    name: "Quant",
    tagline: "Data doesn't lie",
    description: "ML-powered adaptive trader. Uses multi-indicator fusion with online learning.",
    strategy: "Contextual bandit with 7-feature extraction",
    riskLevel: "Medium",
    tradingStyle: "Adaptive, learns from outcomes"
  }
};
```

---

## Phase 3: Trading Dashboard

### Prompt
> "The paper trading system is fully functional. The Trading pages already exist but need to be wired to the new backend APIs. Use the existing TradeSandbox page for paper trading and enhance TradeView for live monitoring."

### Existing Components (in Wraith)
- ✅ `src/pages/TradeView.tsx` - Trading page with chart
- ✅ `src/pages/TradeSandbox.tsx` - Paper trading sandbox
- ✅ `src/components/trading/OrderForm.tsx` - Order entry form
- ✅ `src/components/trading/PositionsTable.tsx` - Open positions
- ✅ `src/components/trading/OrdersTable.tsx` - Orders display
- ✅ `src/pages/Portfolio.tsx` - Portfolio management
- ✅ `HauntClient` - All trading methods already implemented!

### Tasks

#### Step 1: Create/Find Portfolio Selector
- [ ] Check if `PortfolioSelector` exists in `src/components/`
- [ ] If not, create `src/components/trading/PortfolioSelector.tsx`:
  ```tsx
  interface Props {
    userId: string;
    selectedId: string | null;
    onSelect: (portfolioId: string) => void;
  }
  ```
- [ ] Fetch portfolios: `const { data } = useQuery(['portfolios', userId], () => HauntClient.getPortfolios(userId))`
- [ ] Render as dropdown/select using shadcn Select component

#### Step 2: Wire TradeSandbox Page (`src/pages/TradeSandbox.tsx`)
- [ ] Add state for `selectedPortfolioId`
- [ ] Add PortfolioSelector at top of page
- [ ] Fetch portfolio summary when portfolio selected:
  ```tsx
  const { data: summary } = useQuery(
    ['portfolioSummary', selectedPortfolioId],
    () => HauntClient.getPortfolioSummary(selectedPortfolioId!),
    { enabled: !!selectedPortfolioId }
  );
  ```
- [ ] Display summary card with: Cash Balance, Total Value, Unrealized PnL, Margin Used
- [ ] Pass `portfolioId` to child components (OrderForm, PositionsTable, OrdersTable)

#### Step 3: Wire Portfolio Page (`src/pages/Portfolio.tsx`)
- [ ] Add "Create Portfolio" button
- [ ] Create modal/dialog for new portfolio form
- [ ] Form fields: Name (required), Description (optional)
- [ ] Submit handler:
  ```tsx
  const handleCreate = async (data: { name: string; description?: string }) => {
    await HauntClient.createPortfolio({
      userId: currentUser.publicKey,
      name: data.name,
      description: data.description,
    });
    // Invalidate portfolios query to refresh list
  };
  ```
- [ ] Show success toast on creation
- [ ] Refresh portfolio list after creation

#### Step 4: Wire OrderForm (`src/components/trading/OrderForm.tsx`)
- [ ] Receive `portfolioId` as prop
- [ ] Update form schema to match `PlaceOrderRequest`:
  ```typescript
  {
    portfolioId: string;
    symbol: string;
    assetClass: "crypto_spot" | "stock" | "forex";
    side: "buy" | "sell";
    orderType: "market" | "limit" | "stop_loss" | "take_profit";
    quantity: number;
    price?: number;        // for limit orders
    stopLoss?: number;     // optional
    takeProfit?: number;   // optional
    leverage?: number;     // default 1.0
  }
  ```
- [ ] Wire submit: `await HauntClient.placeOrder(formData)`
- [ ] Add leverage slider (1x-10x) for crypto_spot
- [ ] Add optional SL/TP fields (collapsible "Advanced" section)
- [ ] Show success/error toast on submit
- [ ] Reset form after successful submit

#### Step 5: Wire PositionsTable (`src/components/trading/PositionsTable.tsx`)
- [ ] Receive `portfolioId` as prop
- [ ] Fetch positions: `useQuery(['positions', portfolioId], () => HauntClient.getPositions(portfolioId))`
- [ ] Verify columns exist: Symbol, Side, Qty, Entry Price, Current Price, PnL, PnL %
- [ ] Add "Modify" action button per row:
  ```tsx
  const handleModify = async (positionId: string, stopLoss?: number, takeProfit?: number) => {
    await HauntClient.modifyPosition(positionId, { stopLoss, takeProfit });
    // Invalidate positions query
  };
  ```
- [ ] Add "Close" action button per row:
  ```tsx
  const handleClose = async (positionId: string, currentPrice: number) => {
    await HauntClient.closePosition(positionId, currentPrice);
    // Invalidate positions query
  };
  ```
- [ ] Add confirmation dialog before close
- [ ] Style PnL: green for positive, red for negative

#### Step 6: Wire OrdersTable (`src/components/trading/OrdersTable.tsx`)
- [ ] Receive `portfolioId` as prop
- [ ] Fetch orders: `useQuery(['orders', portfolioId], () => HauntClient.getOrders(portfolioId))`
- [ ] Verify columns: Symbol, Type, Side, Qty, Price, Status, Created
- [ ] Add status filter (tabs or dropdown): All, Open, Filled, Cancelled
- [ ] Add "Cancel" button for open orders:
  ```tsx
  const handleCancel = async (orderId: string) => {
    await HauntClient.cancelOrder(orderId);
    // Invalidate orders query
  };
  ```
- [ ] Add confirmation dialog before cancel
- [ ] Format dates: `new Date(createdAt).toLocaleString()`

#### Step 7: Create TradesTable (if needed)
- [ ] Check if trades table exists, if not create `src/components/trading/TradesTable.tsx`
- [ ] Receive `portfolioId` as prop
- [ ] Fetch trades: `useQuery(['trades', portfolioId], () => HauntClient.getTrades(portfolioId))`
- [ ] Columns: Time, Symbol, Side, Qty, Price, Fee, Total
- [ ] Calculate Total: `price * quantity + fee`
- [ ] Sort by executedAt descending (most recent first)

### API Reference

**Portfolio Management**

All portfolio endpoints are under `/api/trading/portfolios`:

```typescript
// GET /api/trading/portfolios?user_id=xxx
// Returns: { data: Portfolio[] }
interface Portfolio {
  id: string;
  userId: string;
  name: string;
  description: string | null;
  baseCurrency: string;
  startingBalance: number;
  cashBalance: number;
  marginUsed: number;
  marginAvailable: number;
  unrealizedPnl: number;
  realizedPnl: number;
  totalValue: number;
  totalTrades: number;
  winningTrades: number;
  costBasisMethod: "fifo" | "lifo" | "average";
  riskSettings: RiskSettings;
  isCompetition: boolean;
  competitionId: string | null;
  createdAt: number;
  updatedAt: number;
}

// POST /api/trading/portfolios
interface CreatePortfolioRequest {
  userId: string;
  name: string;
  description?: string;
  riskSettings?: RiskSettings;
}

// GET /api/trading/portfolios/:id/summary
interface PortfolioSummary {
  portfolioId: string;
  totalValue: number;
  cashBalance: number;
  unrealizedPnl: number;
  realizedPnl: number;
  totalReturnPct: number;
  marginUsed: number;
  marginAvailable: number;
  marginLevel: number;
  openPositions: number;
  openOrders: number;
}
```

**Order Management**

All order endpoints are under `/api/trading/orders`:

```typescript
// GET /api/trading/orders?portfolio_id=xxx
// Returns: { data: Order[] }

// POST /api/trading/orders
interface PlaceOrderRequest {
  portfolioId: string;
  symbol: string;
  assetClass: "crypto_spot" | "stock" | "etf" | "perp" | "option" | "forex";
  side: "buy" | "sell";
  orderType: "market" | "limit" | "stop_loss" | "take_profit" | "trailing_stop";
  quantity: number;
  price?: number;           // Required for limit orders
  stopPrice?: number;       // Required for stop orders
  trailAmount?: number;     // For trailing stop
  trailPercent?: number;    // For trailing stop
  timeInForce?: "gtc" | "fok" | "ioc" | "gtd";
  leverage?: number;        // Default 1.0
  stopLoss?: number;        // Attach SL to position
  takeProfit?: number;      // Attach TP to position
  clientOrderId?: string;
}

// Response
interface Order {
  id: string;
  portfolioId: string;
  symbol: string;
  assetClass: string;
  side: "buy" | "sell";
  orderType: string;
  status: "pending" | "open" | "filled" | "partially_filled" | "cancelled" | "rejected" | "expired";
  quantity: number;
  filledQuantity: number;
  price: number | null;
  avgFillPrice: number | null;
  stopPrice: number | null;
  // ... more fields
  createdAt: number;
  updatedAt: number;
}

// DELETE /api/trading/orders/:id - Cancel order
```

**Position Management**

All position endpoints are under `/api/trading/positions`:

```typescript
// GET /api/trading/positions?portfolio_id=xxx
// Returns: { data: Position[] }
interface Position {
  id: string;
  portfolioId: string;
  symbol: string;
  assetClass: string;
  side: "long" | "short";
  quantity: number;
  entryPrice: number;
  currentPrice: number;
  unrealizedPnl: number;
  unrealizedPnlPct: number;
  realizedPnl: number;
  leverage: number;
  marginUsed: number;
  stopLoss: number | null;
  takeProfit: number | null;
  liquidationPrice: number | null;
  openedAt: number;
  updatedAt: number;
}

// PUT /api/trading/positions/:id - Modify SL/TP
interface ModifyPositionRequest {
  stopLoss?: number;
  takeProfit?: number;
}

// DELETE /api/trading/positions/:id?price=xxx - Close position
```

### UI Components Status
- [ ] `PortfolioSelector` - May need to create or find existing
- [ ] `CreatePortfolioModal` - May need to create
- [ ] `PortfolioSummaryCard` - Check if exists in Portfolio.tsx
- [x] `OrderForm` - **EXISTS**: `src/components/trading/OrderForm.tsx`
- [x] `PositionsTable` - **EXISTS**: `src/components/trading/PositionsTable.tsx`
- [x] `OrdersTable` - **EXISTS**: `src/components/trading/OrdersTable.tsx`
- [ ] `TradesTable` - May need to create (similar to OrdersTable)
- [ ] `SymbolSearch` - Check if exists, or use combobox from shadcn
- [ ] `PnLDisplay` - Utility component for green/red formatting

---

## Phase 4: WebSocket Integration

### Prompt
> "For real-time updates, we need to use the existing HauntSocketProvider and add trading subscriptions. Users should see their positions update in real-time as prices change, and get notifications when orders fill."

### Existing Infrastructure (in Wraith)
- ✅ `src/contexts/HauntSocketProvider.tsx` - WebSocket provider with reconnection
- ✅ `useHauntSocket()` hook - Access socket, subscribe/unsubscribe methods
- ✅ Connection state management already implemented

### Tasks

#### Step 1: Verify WebSocket Provider (`src/contexts/HauntSocketProvider.tsx`)
- [ ] Open `src/contexts/HauntSocketProvider.tsx`
- [ ] Verify `subscribe(channel, params)` method exists
- [ ] Verify `unsubscribe(channel)` method exists
- [ ] Check if 'trading' channel is supported
- [ ] If not, add trading channel support:
  ```tsx
  const subscribe = (channel: string, params?: Record<string, string>) => {
    socket?.send(JSON.stringify({
      type: 'subscribe',
      channel,
      params
    }));
  };
  ```

#### Step 2: Create useTradingSocket Hook (`src/hooks/useTradingSocket.ts`)
- [ ] Create new hook file
- [ ] Subscribe to trading channel on mount:
  ```tsx
  export function useTradingSocket(portfolioId: string) {
    const { socket, connected, subscribe, unsubscribe } = useHauntSocket();
    const [lastUpdate, setLastUpdate] = useState<TradingUpdate | null>(null);

    useEffect(() => {
      if (!connected || !portfolioId) return;
      subscribe('trading', { portfolioId });
      return () => unsubscribe('trading');
    }, [connected, portfolioId]);

    useEffect(() => {
      if (!socket) return;
      const handler = (event: MessageEvent) => {
        const msg = JSON.parse(event.data);
        if (['positionUpdate', 'orderUpdate', 'tradeExecution', 'portfolioUpdate'].includes(msg.type)) {
          setLastUpdate(msg);
        }
      };
      socket.addEventListener('message', handler);
      return () => socket.removeEventListener('message', handler);
    }, [socket]);

    return { lastUpdate };
  }
  ```

#### Step 3: Wire Real-Time Position Updates in PositionsTable
- [ ] Import `useTradingSocket` hook
- [ ] Listen for position updates:
  ```tsx
  const { lastUpdate } = useTradingSocket(portfolioId);

  useEffect(() => {
    if (lastUpdate?.type === 'positionUpdate') {
      // Optimistically update position in local state
      // OR invalidate positions query to refetch
      queryClient.invalidateQueries(['positions', portfolioId]);
    }
  }, [lastUpdate]);
  ```
- [ ] Add CSS transition for PnL changes:
  ```css
  .pnl-flash-positive { animation: flash-green 0.5s; }
  .pnl-flash-negative { animation: flash-red 0.5s; }
  ```
- [ ] Track previous PnL values to detect changes

#### Step 4: Add Order Fill Notifications
- [ ] Import toast system (sonner or react-hot-toast)
- [ ] Listen for order updates in TradeSandbox or parent component:
  ```tsx
  useEffect(() => {
    if (lastUpdate?.type === 'orderUpdate' && lastUpdate.data.updateType === 'filled') {
      toast.success(`Order filled: ${lastUpdate.data.order.symbol} ${lastUpdate.data.order.side.toUpperCase()}`);
      queryClient.invalidateQueries(['orders', portfolioId]);
    }
  }, [lastUpdate]);
  ```
- [ ] Show different toast styles for: filled, cancelled, rejected

#### Step 5: Add Trade Execution Notifications
- [ ] Listen for trade executions:
  ```tsx
  useEffect(() => {
    if (lastUpdate?.type === 'tradeExecution') {
      const { trade, symbol } = lastUpdate.data;
      toast.success(`Trade executed: ${trade.side.toUpperCase()} ${trade.quantity} ${symbol} @ $${trade.price.toFixed(2)}`);
      queryClient.invalidateQueries(['trades', portfolioId]);
    }
  }, [lastUpdate]);
  ```

#### Step 6: Add Portfolio Balance Updates
- [ ] Listen for portfolio updates:
  ```tsx
  useEffect(() => {
    if (lastUpdate?.type === 'portfolioUpdate') {
      queryClient.invalidateQueries(['portfolioSummary', portfolioId]);
    }
  }, [lastUpdate]);
  ```

#### Step 7: Add Margin Warning Notifications
- [ ] Listen for margin warnings:
  ```tsx
  useEffect(() => {
    if (lastUpdate?.type === 'marginWarning') {
      const { symbol, warningLevel } = lastUpdate.data;
      const messages = {
        warning_80: `⚠️ Margin at 80% for ${symbol}`,
        warning_90: `🚨 Margin at 90% for ${symbol}`,
        warning_95: `🔴 CRITICAL: Margin at 95% for ${symbol}`,
        liquidation: `💀 ${symbol} position liquidated!`
      };
      toast.warning(messages[warningLevel]);
    }
  }, [lastUpdate]);
  ```

### WebSocket Protocol

```typescript
// Connect
const ws = new WebSocket('ws://localhost:3000/ws');

// Subscribe to trading updates for a portfolio
ws.send(JSON.stringify({
  type: "subscribe",
  channel: "trading",
  params: { portfolioId: "xxx" }
}));

// Subscribe to peer mesh updates (optional)
ws.send(JSON.stringify({
  type: "subscribe",
  channel: "peers"
}));

// Incoming message types
interface OrderUpdate {
  type: "orderUpdate";
  data: {
    portfolioId: string;
    order: Order;
    updateType: "created" | "filled" | "partially_filled" | "cancelled" | "modified";
  };
}

interface PositionUpdate {
  type: "positionUpdate";
  data: {
    portfolioId: string;
    position: Position;
    updateType: "opened" | "modified" | "closed" | "liquidated" | "price_updated";
  };
}

interface TradeExecution {
  type: "tradeExecution";
  data: {
    portfolioId: string;
    trade: Trade;
    orderId: string;
    symbol: string;
  };
}

interface PortfolioUpdate {
  type: "portfolioUpdate";
  data: {
    portfolioId: string;
    cashBalance: number;
    marginUsed: number;
    unrealizedPnl: number;
    realizedPnl: number;
    totalValue: number;
    updateType: "balance_changed" | "margin_changed";
    timestamp: number;
  };
}

interface MarginWarning {
  type: "marginWarning";
  data: {
    portfolioId: string;
    positionId: string;
    symbol: string;
    marginLevel: number;
    warningLevel: "warning_80" | "warning_90" | "warning_95" | "liquidation";
  };
}

interface LiquidationAlert {
  type: "liquidationAlert";
  data: {
    portfolioId: string;
    positionId: string;
    symbol: string;
    liquidationPrice: number;
    lossAmount: number;
  };
}
```

---

## Phase 5: Navigation & Layout Updates

### Prompt
> "Most routes already exist in Wraith. We just need to add the Bots page and ensure navigation is connected."

### Existing Routes (check `src/App.tsx`)
- ✅ `/leaderboard` - Already exists
- ✅ `/trade` - TradeView exists
- ✅ `/trade-sandbox` - TradeSandbox exists
- ✅ `/portfolio` - Portfolio exists
- ❌ `/bots` - NEW - needs to be added

### Tasks

#### Step 1: Add Bot Routes (`src/App.tsx`)
- [ ] Import page components:
  ```tsx
  import { BotsPage } from './pages/BotsPage';
  import { BotDetailPage } from './pages/BotDetailPage';
  ```
- [ ] Add routes inside Router:
  ```tsx
  <Route path="/bots" element={<BotsPage />} />
  <Route path="/bots/:botId" element={<BotDetailPage />} />
  ```
- [ ] Verify routes work by visiting `/bots` in browser

#### Step 2: Find Navigation Component
- [ ] Search for nav component: `grep -r "NavLink\|navigation" src/components/`
- [ ] Common locations: `src/components/layout/`, `src/components/nav/`, `src/components/Sidebar.tsx`
- [ ] Identify where nav links are defined

#### Step 3: Add Bots Link to Navigation
- [ ] Import Bot icon: `import { Bot } from 'lucide-react';`
- [ ] Add nav item:
  ```tsx
  <NavLink to="/bots" className="...">
    <Bot className="h-4 w-4" />
    <span>Bots</span>
  </NavLink>
  ```
- [ ] Position near Trading/Leaderboard links

#### Step 4: Verify Existing Links
- [ ] Check "Leaderboard" link exists and points to `/leaderboard`
- [ ] Check "Trade" link exists and points to `/trade` or `/trade-sandbox`
- [ ] Check "Portfolio" link exists and points to `/portfolio`
- [ ] Fix any broken links

#### Step 5: Update Mobile Navigation
- [ ] Find mobile nav component (often separate file or media query)
- [ ] Add Bots link to mobile menu
- [ ] Test on mobile viewport (F12 → device mode)
- [ ] Ensure hamburger menu includes all new routes

---

## Phase 6: User Authentication Integration

### Prompt
> "The trading system needs to know which user is making requests. We use Solana wallet authentication. Make sure all trading API calls include the user's public key and that portfolios are filtered by user."

### Tasks

#### Step 1: Find Existing Auth Implementation
- [ ] Search for wallet connection: `grep -r "useWallet\|wallet" src/`
- [ ] Check for auth context: `src/contexts/AuthContext.tsx` or similar
- [ ] Check for auth store: `src/stores/authStore.ts` or similar
- [ ] Identify how user's public key is accessed

#### Step 2: Create useCurrentUser Hook (if needed)
- [ ] Create `src/hooks/useCurrentUser.ts`:
  ```tsx
  export function useCurrentUser() {
    const { publicKey, connected } = useWallet(); // or your auth method

    return {
      userId: publicKey?.toString() || null,
      isConnected: connected,
      isBot: false,
    };
  }
  ```

#### Step 3: Wire Auth to Portfolio Requests
- [ ] In TradeSandbox/Portfolio pages, get userId:
  ```tsx
  const { userId, isConnected } = useCurrentUser();

  // Only fetch portfolios if connected
  const { data: portfolios } = useQuery(
    ['portfolios', userId],
    () => HauntClient.getPortfolios(userId!),
    { enabled: !!userId }
  );
  ```
- [ ] Show "Connect Wallet" prompt if not connected
- [ ] Pass userId to create portfolio requests

#### Step 4: Handle Portfolio Ownership
- [ ] Filter portfolios in selector to show only user's:
  ```tsx
  const myPortfolios = portfolios?.filter(p => p.userId === userId);
  ```
- [ ] Bot portfolios (userId starts with `bot_`) are view-only
- [ ] Disable edit/trade buttons for bot portfolios:
  ```tsx
  const isOwner = portfolio.userId === userId;
  <Button disabled={!isOwner}>Place Order</Button>
  ```

#### Step 5: Handle Unauthenticated States
- [ ] Redirect to login/connect if trying to access trading pages without auth
- [ ] Show read-only mode for leaderboard/bots pages
- [ ] Add "Connect to Trade" CTA on trading pages when not connected

### Auth API Reference

```typescript
// GET /api/auth/challenge?public_key=xxx
interface ChallengeResponse {
  message: string;
  nonce: string;
}

// POST /api/auth/verify
interface VerifyRequest {
  publicKey: string;
  signature: string;
  message: string;
}

interface VerifyResponse {
  success: boolean;
  token: string;
  profile: UserProfile;
}

// GET /api/auth/me (with Authorization header)
interface UserProfile {
  publicKey: string;
  username: string | null;
  avatar: string | null;
  bio: string | null;
  createdAt: number;
}
```

---

## Error Handling

### Trading Errors

```typescript
interface ErrorResponse {
  error: string;
  code: string;
}

// Error codes to handle
const TRADING_ERRORS = {
  PORTFOLIO_NOT_FOUND: "Portfolio not found",
  ORDER_NOT_FOUND: "Order not found",
  POSITION_NOT_FOUND: "Position not found",
  INSUFFICIENT_FUNDS: "Not enough cash to place order",
  INSUFFICIENT_MARGIN: "Not enough margin available",
  POSITION_LIMIT_EXCEEDED: "Maximum positions reached",
  INVALID_ORDER: "Order parameters invalid",
  CANNOT_CANCEL_ORDER: "Order already filled/cancelled",
  LEVERAGE_EXCEEDED: "Requested leverage too high",
  PORTFOLIO_STOPPED: "Portfolio hit stop-loss, trading disabled",
  NO_PRICE_DATA: "No price data for this symbol"
};
```

---

## Testing Checklist

### Bots Page
- [ ] List all bots displays correctly
- [ ] Bot status shows running/stopped
- [ ] Performance metrics load and display
- [ ] Trade history loads with pagination
- [ ] Follow/Unfollow works with correct toast
- [ ] Bot badges show correct personality

### Leaderboard Page
- [ ] Leaderboard loads and sorts correctly
- [ ] Bot entries have special badge
- [ ] Click-through to portfolio works
- [ ] Handles empty state gracefully

### Trading Dashboard
- [ ] Create portfolio works
- [ ] Portfolio selector shows user's portfolios
- [ ] Place market order works
- [ ] Place limit order works
- [ ] Cancel order works
- [ ] Close position works
- [ ] Modify SL/TP works
- [ ] Real-time updates via WebSocket work

### Integration
- [ ] Auth state persists across pages
- [ ] WebSocket reconnects on disconnect
- [ ] Error toasts show for API failures
- [ ] Loading states show during fetches

---

## Priority Order

1. **High Priority** (Core functionality)
   - [ ] Trading dashboard with portfolio management
   - [ ] Order entry and position management
   - [ ] WebSocket for real-time updates

2. **Medium Priority** (Social features)
   - [ ] Bots page with list and detail views
   - [ ] Follow bot functionality
   - [ ] Leaderboard page

3. **Lower Priority** (Polish)
   - [ ] Advanced charts for positions
   - [ ] Trade analytics
   - [ ] Copy trading automation

---

## Notes for Implementation

### Wraith Already Has
1. **API Client**: `HauntClient` in `src/services/hauntClient.ts` - use this for all API calls
2. **WebSocket**: `HauntSocketProvider` in `src/contexts/` - already connected, use `useHauntSocket()` hook
3. **State**: Zustand stores exist - check `src/stores/` for existing patterns
4. **UI Components**: shadcn/ui is installed - use existing components from `src/components/ui/`
5. **Toast**: Check for existing toast setup (likely sonner or react-hot-toast)

### Implementation Tips
1. Follow existing patterns in Wraith for consistency
2. Check `src/types/` for existing TypeScript interfaces
3. Use existing `HauntClient` methods instead of raw fetch
4. Check `src/hooks/` for existing custom hooks that may be useful

---

## Backend Status

All backend APIs are complete:

- [x] `GET /api/trading/leaderboard` - Added to `src/api/trading.rs`
- [x] All bot endpoints in `src/api/bots.rs`
- [x] All trading endpoints in `src/api/trading.rs`

---

---

## Summary: What's Done vs What's Needed

### ✅ Already Done (Wraith)
- Leaderboard page with TraderRow component
- TradeView and TradeSandbox pages
- OrderForm, PositionsTable, OrdersTable components
- Portfolio page
- HauntClient with all trading API methods
- HauntSocketProvider for WebSocket
- Routes for /leaderboard, /trade, /trade-sandbox, /portfolio

### ❌ Still Needed
- Bots page (`/bots` and `/bots/:botId`) - NEW pages
- Wire existing components to HauntClient methods
- Add trading channel subscription to WebSocket
- Add bot badges to leaderboard
- Add real-time position updates

### 📊 Effort Estimate
- **Leaderboard**: Minor updates (add bot badges, wire to API)
- **Bots Page**: Medium effort (new page, but similar patterns exist)
- **Trading Dashboard**: Mostly wiring (components exist!)
- **WebSocket**: Minor updates (infrastructure exists)

---

*Last Updated: 2025-02-04*
*Backend Version: Grannybot branch @ commit 9be34fe*
