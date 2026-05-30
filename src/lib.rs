use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

// ── Data point ───────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataPoint {
    pub id: Uuid,
    pub sensor_id: String,
    pub value: f64,
    pub timestamp: u64,
    pub tags: HashMap<String, String>,
}

impl DataPoint {
    pub fn new(sensor_id: &str, value: f64, timestamp: u64) -> Self {
        DataPoint {
            id: Uuid::new_v4(),
            sensor_id: sensor_id.to_string(),
            value,
            timestamp,
            tags: HashMap::new(),
        }
    }

    pub fn with_tag(mut self, k: &str, v: &str) -> Self {
        self.tags.insert(k.to_string(), v.to_string());
        self
    }
}

// ── Time range ───────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeRange {
    pub start: u64,
    pub end: u64,
}

impl TimeRange {
    pub fn new(start: u64, end: u64) -> Self {
        TimeRange { start, end }
    }

    pub fn contains(&self, ts: u64) -> bool {
        ts >= self.start && ts <= self.end
    }

    pub fn duration_ms(&self) -> u64 {
        self.end.saturating_sub(self.start)
    }
}

// ── Query ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HistoryQuery {
    pub sensor_id: Option<String>,
    pub time_range: Option<TimeRange>,
    pub tag_filters: HashMap<String, String>,
    pub limit: Option<usize>,
}

impl HistoryQuery {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn for_sensor(mut self, sensor_id: &str) -> Self {
        self.sensor_id = Some(sensor_id.to_string());
        self
    }

    pub fn in_range(mut self, start: u64, end: u64) -> Self {
        self.time_range = Some(TimeRange::new(start, end));
        self
    }

    pub fn with_tag(mut self, k: &str, v: &str) -> Self {
        self.tag_filters.insert(k.to_string(), v.to_string());
        self
    }

    pub fn limit(mut self, n: usize) -> Self {
        self.limit = Some(n);
        self
    }

    pub fn matches(&self, dp: &DataPoint) -> bool {
        if let Some(ref sid) = self.sensor_id {
            if dp.sensor_id != *sid {
                return false;
            }
        }
        if let Some(ref tr) = self.time_range {
            if !tr.contains(dp.timestamp) {
                return false;
            }
        }
        for (k, v) in &self.tag_filters {
            if dp.tags.get(k) != Some(v) {
                return false;
            }
        }
        true
    }
}

// ── Aggregation ──────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Aggregation {
    Min,
    Max,
    Avg,
    Sum,
}

// ── Time series ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeries {
    pub sensor_id: String,
    pub points: Vec<DataPoint>,
}

impl TimeSeries {
    pub fn new(sensor_id: &str) -> Self {
        TimeSeries {
            sensor_id: sensor_id.to_string(),
            points: Vec::new(),
        }
    }

    /// Insert a data point, maintaining sorted order by timestamp.
    pub fn insert(&mut self, dp: DataPoint) {
        let pos = self.points.partition_point(|p| p.timestamp < dp.timestamp);
        self.points.insert(pos, dp);
    }

    /// Query points matching the given query.
    pub fn query_range(&self, query: &HistoryQuery) -> Vec<&DataPoint> {
        let mut results: Vec<&DataPoint> = self
            .points
            .iter()
            .filter(|p| query.matches(p))
            .collect();

        if let Some(limit) = query.limit {
            results.truncate(limit);
        }
        results
    }

    /// Get the latest data point.
    pub fn latest(&self) -> Option<&DataPoint> {
        self.points.last()
    }

    /// Aggregate values using the specified function.
    pub fn aggregate(&self, agg: Aggregation) -> Option<f64> {
        if self.points.is_empty() {
            return None;
        }
        match agg {
            Aggregation::Min => self.points.iter().map(|p| p.value).reduce(f64::min),
            Aggregation::Max => self.points.iter().map(|p| p.value).reduce(f64::max),
            Aggregation::Sum => Some(self.points.iter().map(|p| p.value).sum()),
            Aggregation::Avg => {
                let sum: f64 = self.points.iter().map(|p| p.value).sum();
                Some(sum / self.points.len() as f64)
            }
        }
    }

    /// Downsample to at most `n` points by evenly sampling.
    pub fn downsample(&self, n: usize) -> TimeSeries {
        if n >= self.points.len() {
            return self.clone();
        }
        let step = self.points.len() as f64 / n as f64;
        let mut sampled = Vec::with_capacity(n);
        let mut i = 0.0;
        while i < self.points.len() as f64 {
            sampled.push(self.points[i as usize].clone());
            i += step;
        }
        TimeSeries {
            sensor_id: self.sensor_id.clone(),
            points: sampled,
        }
    }

    pub fn len(&self) -> usize {
        self.points.len()
    }

    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }
}

// ── History store ────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HistoryStore {
    series: HashMap<String, TimeSeries>,
}

impl HistoryStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a data point into the appropriate time series.
    pub fn insert(&mut self, dp: DataPoint) {
        let series = self
            .series
            .entry(dp.sensor_id.clone())
            .or_insert_with(|| TimeSeries::new(&dp.sensor_id));
        series.insert(dp);
    }

    /// Query across all series matching the query.
    pub fn query_range(&self, query: &HistoryQuery) -> Vec<&DataPoint> {
        let mut results = Vec::new();
        if let Some(ref sensor_id) = query.sensor_id {
            if let Some(series) = self.series.get(sensor_id) {
                results.extend(series.query_range(query));
            }
        } else {
            for series in self.series.values() {
                results.extend(series.query_range(query));
            }
        }
        if let Some(limit) = query.limit {
            results.truncate(limit);
        }
        results
    }

    /// Get the latest point for a sensor.
    pub fn latest(&self, sensor_id: &str) -> Option<&DataPoint> {
        self.series.get(sensor_id).and_then(|s| s.latest())
    }

    /// Aggregate over a specific sensor's data.
    pub fn aggregate(&self, sensor_id: &str, agg: Aggregation) -> Option<f64> {
        self.series.get(sensor_id).and_then(|s| s.aggregate(agg))
    }

    /// Downsample a specific sensor's data.
    pub fn downsample(&self, sensor_id: &str, n: usize) -> Option<TimeSeries> {
        self.series.get(sensor_id).map(|s| s.downsample(n))
    }

    /// Get a time series for a sensor.
    pub fn series(&self, sensor_id: &str) -> Option<&TimeSeries> {
        self.series.get(sensor_id)
    }

    pub fn sensor_count(&self) -> usize {
        self.series.len()
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn dp(sensor: &str, value: f64, ts: u64) -> DataPoint {
        DataPoint::new(sensor, value, ts)
    }

    #[test]
    fn data_point_construction() {
        let p = dp("temp", 22.5, 1000);
        assert_eq!(p.sensor_id, "temp");
        assert!((p.value - 22.5).abs() < 1e-9);
        assert_eq!(p.timestamp, 1000);
    }

    #[test]
    fn data_point_with_tags() {
        let p = dp("s1", 1.0, 0).with_tag("room", "kitchen");
        assert_eq!(p.tags.get("room").unwrap(), "kitchen");
    }

    #[test]
    fn time_range_contains() {
        let tr = TimeRange::new(100, 200);
        assert!(tr.contains(100));
        assert!(tr.contains(150));
        assert!(tr.contains(200));
        assert!(!tr.contains(99));
        assert!(!tr.contains(201));
    }

    #[test]
    fn time_range_duration() {
        let tr = TimeRange::new(1000, 5000);
        assert_eq!(tr.duration_ms(), 4000);
    }

    #[test]
    fn insert_maintains_sort() {
        let mut ts = TimeSeries::new("s1");
        ts.insert(dp("s1", 3.0, 300));
        ts.insert(dp("s1", 1.0, 100));
        ts.insert(dp("s1", 2.0, 200));
        assert_eq!(ts.points[0].timestamp, 100);
        assert_eq!(ts.points[1].timestamp, 200);
        assert_eq!(ts.points[2].timestamp, 300);
    }

    #[test]
    fn query_range_basic() {
        let mut ts = TimeSeries::new("s1");
        ts.insert(dp("s1", 1.0, 100));
        ts.insert(dp("s1", 2.0, 200));
        ts.insert(dp("s1", 3.0, 300));
        ts.insert(dp("s1", 4.0, 400));

        let query = HistoryQuery::new().in_range(150, 350);
        let results = ts.query_range(&query);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn query_range_with_limit() {
        let mut ts = TimeSeries::new("s1");
        for i in 0..10 {
            ts.insert(dp("s1", i as f64, i * 100));
        }
        let query = HistoryQuery::new().limit(3);
        let results = ts.query_range(&query);
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn query_with_tag_filter() {
        let mut ts = TimeSeries::new("s1");
        ts.insert(dp("s1", 1.0, 100).with_tag("room", "kitchen"));
        ts.insert(dp("s1", 2.0, 200).with_tag("room", "bedroom"));
        ts.insert(dp("s1", 3.0, 300).with_tag("room", "kitchen"));

        let query = HistoryQuery::new().with_tag("room", "kitchen");
        let results = ts.query_range(&query);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn latest_point() {
        let mut ts = TimeSeries::new("s1");
        ts.insert(dp("s1", 1.0, 100));
        ts.insert(dp("s1", 2.0, 300));
        ts.insert(dp("s1", 3.0, 200));
        let latest = ts.latest().unwrap();
        assert_eq!(latest.timestamp, 300);
        assert!((latest.value - 2.0).abs() < 1e-9);
    }

    #[test]
    fn latest_empty() {
        let ts = TimeSeries::new("s1");
        assert!(ts.latest().is_none());
    }

    #[test]
    fn aggregate_min() {
        let mut ts = TimeSeries::new("s1");
        ts.insert(dp("s1", 5.0, 100));
        ts.insert(dp("s1", 2.0, 200));
        ts.insert(dp("s1", 8.0, 300));
        assert!((ts.aggregate(Aggregation::Min).unwrap() - 2.0).abs() < 1e-9);
    }

    #[test]
    fn aggregate_max() {
        let mut ts = TimeSeries::new("s1");
        ts.insert(dp("s1", 5.0, 100));
        ts.insert(dp("s1", 2.0, 200));
        ts.insert(dp("s1", 8.0, 300));
        assert!((ts.aggregate(Aggregation::Max).unwrap() - 8.0).abs() < 1e-9);
    }

    #[test]
    fn aggregate_avg() {
        let mut ts = TimeSeries::new("s1");
        ts.insert(dp("s1", 10.0, 100));
        ts.insert(dp("s1", 20.0, 200));
        ts.insert(dp("s1", 30.0, 300));
        assert!((ts.aggregate(Aggregation::Avg).unwrap() - 20.0).abs() < 1e-9);
    }

    #[test]
    fn aggregate_sum() {
        let mut ts = TimeSeries::new("s1");
        ts.insert(dp("s1", 10.0, 100));
        ts.insert(dp("s1", 20.0, 200));
        assert!((ts.aggregate(Aggregation::Sum).unwrap() - 30.0).abs() < 1e-9);
    }

    #[test]
    fn aggregate_empty() {
        let ts = TimeSeries::new("s1");
        assert!(ts.aggregate(Aggregation::Min).is_none());
    }

    #[test]
    fn downsample() {
        let mut ts = TimeSeries::new("s1");
        for i in 0..100 {
            ts.insert(dp("s1", i as f64, i as u64 * 1000));
        }
        let downsampled = ts.downsample(10);
        assert_eq!(downsampled.len(), 10);
        assert_eq!(downsampled.sensor_id, "s1");
    }

    #[test]
    fn downsample_larger_than_data() {
        let mut ts = TimeSeries::new("s1");
        ts.insert(dp("s1", 1.0, 100));
        ts.insert(dp("s1", 2.0, 200));
        let downsampled = ts.downsample(10);
        assert_eq!(downsampled.len(), 2);
    }

    #[test]
    fn history_store_insert_and_query() {
        let mut store = HistoryStore::new();
        store.insert(dp("temp", 22.0, 1000));
        store.insert(dp("temp", 23.0, 2000));
        store.insert(dp("humidity", 55.0, 1500));

        let query = HistoryQuery::new().for_sensor("temp");
        let results = store.query_range(&query);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn history_store_latest() {
        let mut store = HistoryStore::new();
        store.insert(dp("temp", 22.0, 1000));
        store.insert(dp("temp", 23.0, 2000));
        let latest = store.latest("temp").unwrap();
        assert!((latest.value - 23.0).abs() < 1e-9);
    }

    #[test]
    fn history_store_aggregate() {
        let mut store = HistoryStore::new();
        store.insert(dp("temp", 20.0, 1000));
        store.insert(dp("temp", 25.0, 2000));
        assert!((store.aggregate("temp", Aggregation::Avg).unwrap() - 22.5).abs() < 1e-9);
    }

    #[test]
    fn serialization_roundtrip_data_point() {
        let p = dp("s1", 42.0, 1000).with_tag("env", "test");
        let json = serde_json::to_string(&p).unwrap();
        let back: DataPoint = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, p.id);
        assert_eq!(back.sensor_id, p.sensor_id);
        assert_eq!(back.tags, p.tags);
    }

    #[test]
    fn serialization_roundtrip_time_series() {
        let mut ts = TimeSeries::new("s1");
        ts.insert(dp("s1", 1.0, 100));
        ts.insert(dp("s1", 2.0, 200));
        let json = serde_json::to_string(&ts).unwrap();
        let back: TimeSeries = serde_json::from_str(&json).unwrap();
        assert_eq!(back.sensor_id, "s1");
        assert_eq!(back.points.len(), 2);
    }
}
