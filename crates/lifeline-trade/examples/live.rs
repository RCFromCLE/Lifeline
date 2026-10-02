//! Live check against the real trade site and poe.ninja (a few requests).
//! cargo run -p lifeline-trade --example live -- "HC Forbidden Rites"

use lifeline_trade::Market;
use serde_json::json;

fn main() {
    let league = std::env::args().nth(1).unwrap_or_else(|| "HC Forbidden Rites".into());
    let m = Market::new();

    let stats = m.find_stats("cold res", 3).expect("stats");
    println!(
        "stat lookup 'cold res': {:?}",
        stats.iter().map(|s| (&s.id, &s.text)).collect::<Vec<_>>()
    );

    let query = json!({
        "query": {
            "status": {"option": "securable"},
            "stats": [{"type": "and", "filters": [{"id": stats[0].id, "value": {"min": 20}}]}],
            "filters": {"type_filters": {"filters": {"category": {"option": "armour.boots"}}}}
        }
    });
    match m.search(&league, &query, 5) {
        Ok(r) => {
            println!(
                "\n{} instant-buyout boots with ≥20% cold res in {league}; first {}:",
                r.total,
                r.listings.len()
            );
            for l in &r.listings {
                println!(
                    "  {} {} | {} | ib={} | {}",
                    l.name,
                    l.base,
                    l.price.as_deref().unwrap_or("no price"),
                    l.instant_buyout,
                    l.mods.join("; ")
                );
            }
            println!("open: {}", r.url);
        }
        Err(e) => println!("search failed: {e}"),
    }

    for name in ["Divine Orb", "Exalted Orb"] {
        match m.price(&league, name) {
            Ok(hits) => println!("\nprice '{name}': {:?}", hits.iter().take(2).collect::<Vec<_>>()),
            Err(e) => println!("price failed: {e}"),
        }
    }
}
