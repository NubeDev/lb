//! How much disc does one committed sample actually cost? Grounds the FIFO cap's default.
use lb_ingest::{commit_batch, write, Qos, Sample};
use lb_store::Store;
use serde_json::json;

async fn dir_bytes(p: &str) -> u64 {
    fn walk(p: &std::path::Path) -> u64 {
        let mut t = 0;
        if let Ok(rd) = std::fs::read_dir(p) {
            for e in rd.flatten() {
                let m = e.metadata().unwrap();
                t += if m.is_dir() { walk(&e.path()) } else { m.len() };
            }
        }
        t
    }
    walk(std::path::Path::new(p))
}

#[tokio::main]
async fn main() {
    let base = std::env::args().nth(1).expect("path");
    for n in [10_000u64, 50_000] {
        let dir = format!("{base}/n{n}");
        let store = Store::open(&dir).await.unwrap();
        let rows: Vec<Sample> = (1..=n).map(|seq| Sample {
            series: format!("fleet.sensor{}", seq % 50),
            producer: format!("node-{}", seq % 10),
            ts: 1_784_070_000_000 + seq * 1000,
            seq,
            payload: json!(seq as f64 * 0.1),
            labels: Default::default(),
            qos: Qos::BestEffort,
        }).collect();
        for c in rows.chunks(2000) {
            write(&store, "acme", c, 0).await.unwrap();
            loop { if commit_batch(&store, "acme", 256).await.unwrap().committed == 0 { break } }
        }
        drop(store);
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        let b = dir_bytes(&dir).await;
        println!("{n:7} samples committed → {:8.1} MB on disc  ({:.0} bytes/sample)",
            b as f64 / 1e6, b as f64 / n as f64);
    }
}
