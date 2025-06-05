use std::time::Instant;
use std::fs::{OpenOptions};
use std::io::Write;
use serde::Serialize;

/// Đo thời gian bắt đầu
pub fn measure_start(label: &str) -> (String, Instant) {
    (label.to_string(), Instant::now())
}

/// Đo thời gian kết thúc và in ra stdout
pub fn measure_end(start: (String, Instant)) {
    let elapsed = start.1.elapsed();
    println!("Elapsed: {:.2?} for '{}'", elapsed, start.0);
}

/// Cấu trúc JSON log kết quả quote
#[derive(Serialize)]
pub struct QuoteLog {
    pub chain: String,
    pub method: String,
    pub volume: String,
    pub from_token: String,
    pub to_token: String,
    pub amount_out: String,
    pub elapsed_ms: u128,
}

/// Log quote ra stdout và file (tuỳ chọn)
pub fn log_quote(log: QuoteLog) {
    let json = serde_json::to_string(&log).unwrap();
    println!("{json}");

    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open("output/quote.jsonl")
        .unwrap();

    writeln!(file, "{json}").unwrap();
}
