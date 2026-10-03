//! Prints one raw trade listing (item + listing JSON) and a few currency
//! entries from the trade site's static data, to check field names.
//! `cargo run --example raw_listing -- "<league>"`
fn main() {
    let league = std::env::args().nth(1).unwrap_or_else(|| "HC Forbidden Rites".into());
    let agent = ureq::Agent::new_with_defaults();
    let base = "https://www.pathofexile.com";
    let body = serde_json::json!({"query": {"status": {"option": "securable"}, "type": "Iron Buckler"}, "sort": {"price": "asc"}});
    let seg = league.replace(' ', "%20");
    let mut r = agent.post(&format!("{base}/api/trade2/search/poe2/{seg}")).header("User-Agent", "Lifeline dev check").header("Content-Type", "application/json").send(body.to_string()).unwrap();
    let found: serde_json::Value = serde_json::from_str(&r.body_mut().read_to_string().unwrap()).unwrap();
    let id = found["result"][0].as_str().unwrap();
    let q = found["id"].as_str().unwrap();
    let mut f = agent.get(&format!("{base}/api/trade2/fetch/{id}?query={q}&realm=poe2")).header("User-Agent", "Lifeline dev check").call().unwrap();
    let fetched: serde_json::Value = serde_json::from_str(&f.body_mut().read_to_string().unwrap()).unwrap();
    let mut item = fetched["result"][0].clone();
    item["listing"]["hideout_token"] = serde_json::Value::Null;
    item["listing"]["whisper_token"] = serde_json::Value::Null;
    println!("{}", serde_json::to_string_pretty(&item).unwrap());
    let mut s = agent.get(&format!("{base}/api/trade2/data/static")).header("User-Agent", "Lifeline dev check").call().unwrap();
    let stat: serde_json::Value = serde_json::from_str(&s.body_mut().read_to_string().unwrap()).unwrap();
    for group in stat["result"].as_array().unwrap().iter().take(1) {
        println!("group {}: {}", group["id"], serde_json::to_string(&group["entries"].as_array().unwrap().iter().take(4).collect::<Vec<_>>()).unwrap());
    }
}
