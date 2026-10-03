//! Live check: does the trade site rank by a property (total Armour)?
//! `cargo run --example sort_check -- "<league>"`
use serde_json::json;

fn main() {
    let league = std::env::args().nth(1).unwrap_or_else(|| "HC Forbidden Rites".into());
    let market = lifeline_trade::Market::new();
    let body = json!({
        "query": {"status": {"option": "securable"}, "filters": {
            "type_filters": {"filters": {"category": {"option": "armour.chest"}}},
            "req_filters": {"filters": {"lvl": {"max": 20}}},
            "equipment_filters": {"filters": {"ev": {"min": 1}}}}},
        "sort": {"ar": "desc"}
    });
    match market.search(&league, &body, 5) {
        Ok(o) => {
            println!("ok: {} listed", o.total);
            for l in &o.listings {
                let ar: Vec<&String> = l.mods.iter().filter(|m| m.contains("Armour") || m.contains("Evasion")).collect();
                println!("  {} | {} | {:?}", l.base, l.price.clone().unwrap_or_default(), ar);
                println!("    class {:?} props {:?} granted {:?} totals {:?}", l.item_class, l.properties, l.granted, l.totals);
                println!("    price {:?} × {:?} icon {} fee {:?} note {:?}", l.price_amount, l.currency_name, l.currency_icon.is_some(), l.fee, l.note);
            }
        }
        Err(e) => println!("error: {e}"),
    }
}
