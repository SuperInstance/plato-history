# plato-history

> Historical data storage and time-series queries for PLATO room readings

## What This Does

plato-history stores sensor readings as time series and provides efficient range queries, aggregations, and downsampling. Data points are kept sorted by timestamp. Queries can filter by sensor, time range, tags, and limit.

## The Key Idea

Every sensor reading is a `(timestamp, value)` pair. Over time, you get millions of them. plato-history organizes them per sensor, keeps them sorted, and lets you ask: "What were the kitchen temperatures last Tuesday?" or "What's the average humidity this month?" or "Give me 10 evenly-spaced samples from the last 24 hours."

## Install

```bash
cargo add plato-history
```

## Quick Start

```rust
use plato_history::{HistoryStore, DataPoint, HistoryQuery, Aggregation};

let mut store = HistoryStore::new();
store.insert(DataPoint::new("temp", 22.0, 1000));
store.insert(DataPoint::new("temp", 23.5, 2000));
store.insert(DataPoint::new("temp", 21.0, 3000).with_tag("room", "kitchen"));

// Query by time range
let results = store.query_range(
    HistoryQuery::new().for_sensor("temp").in_range(1500, 2500)
);

// Aggregate
let avg = store.aggregate("temp", Aggregation::Avg);

// Downsample 100 points to 10
let downsampled = store.downsample("temp", 10);
```

## API Reference

| Type | Description |
|---|---|
| `DataPoint { sensor_id, value, timestamp, tags }` | Single reading. Builder: `new(id, val, ts).with_tag(k, v)` |
| `TimeRange { start, end }` | Timestamp range. `contains(ts)`, `duration_ms()` |
| `HistoryQuery` | Builder: `for_sensor()`, `in_range()`, `with_tag()`, `limit()` |
| `Aggregation` | `Min` / `Max` / `Avg` / `Sum` |
| `TimeSeries` | Per-sensor sorted data. `insert()`, `query_range()`, `latest()`, `aggregate()`, `downsample(n)` |
| `HistoryStore` | Multi-sensor store. `insert()`, `query_range()`, `latest()`, `aggregate()`, `downsample()` |

## Testing

22 tests: data point construction, time range, sorted insertion, range queries, tag filtering, limits, latest point, all aggregations, downsampling, store operations, serialization.

## License

Apache-2.0
