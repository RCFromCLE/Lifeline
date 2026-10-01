//! Path of Building build codes and share links.
//!
//! Encoding (PoB2 `src/Classes/ImportTab.lua`): `base64(Deflate(xml))` with
//! `+`→`-` and `/`→`_`. `Deflate` is zlib-wrapped level 9; PoB's `Inflate`
//! also auto-detects gzip. Real codes come both padded and unpadded, so
//! decoding accepts either.

use std::io::{Read, Write};

use base64::alphabet;
use base64::engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig};
use base64::Engine as _;
use flate2::read::{MultiGzDecoder, ZlibDecoder};
use flate2::write::ZlibEncoder;
use flate2::Compression;

use crate::Error;

const LENIENT: GeneralPurpose = GeneralPurpose::new(
    &alphabet::URL_SAFE,
    GeneralPurposeConfig::new()
        .with_decode_padding_mode(DecodePaddingMode::Indifferent)
        .with_decode_allow_trailing_bits(true),
);

const PADDED: GeneralPurpose = GeneralPurpose::new(&alphabet::URL_SAFE, GeneralPurposeConfig::new());

/// Build code → PoB XML. Whitespace is ignored and both base64 alphabets are
/// accepted, matching PoB's own tolerance.
pub fn decode(code: &str) -> Result<String, Error> {
    let cleaned: String = code
        .chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| match c {
            '+' => '-',
            '/' => '_',
            c => c,
        })
        .collect();
    let bytes = LENIENT.decode(cleaned.as_bytes())?;
    let mut xml = String::new();
    if bytes.starts_with(&[0x1f, 0x8b]) {
        MultiGzDecoder::new(bytes.as_slice())
            .read_to_string(&mut xml)
            .map_err(Error::Inflate)?;
    } else {
        ZlibDecoder::new(bytes.as_slice())
            .read_to_string(&mut xml)
            .map_err(Error::Inflate)?;
    }
    Ok(xml)
}

/// PoB XML → build code, the way PoB exports it.
pub fn encode(xml: &str) -> String {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(xml.as_bytes()).expect("writing to a Vec cannot fail");
    let bytes = encoder.finish().expect("writing to a Vec cannot fail");
    PADDED.encode(bytes)
}

/// Where a pasted build comes from and how to get its data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildSource {
    /// The input is not a link; treat it as a build code.
    Code(String),
    /// Download this URL; the body is a build code. URL patterns follow PoB2's
    /// own importer (`src/Modules/BuildSiteTools.lua`).
    CodeUrl(String),
    /// Download this URL; the body is Maxroll planner JSON. Undocumented
    /// endpoint used by maxroll.gg itself — experimental, may break.
    MaxrollPlannerUrl(String),
    /// A site we deliberately don't fetch from, with what to do instead.
    Unsupported { site: &'static str, advice: &'static str },
}

/// Classifies whatever the user pasted: a raw code, a share link, a
/// `pob2://` protocol link, or a YouTube/Google redirect wrapping one.
pub fn resolve(input: &str) -> BuildSource {
    let input = input.trim();
    let unwrapped = unwrap_redirect(input);
    let input = unwrapped.as_deref().unwrap_or(input);

    if let Some(id) = input.strip_prefix("pob2://pobbin/") {
        return BuildSource::CodeUrl(format!("https://pobb.in/pob/{}", first_segment(id)));
    }
    let Some(rest) = input.strip_prefix("https://").or_else(|| input.strip_prefix("http://")) else {
        return BuildSource::Code(input.to_owned());
    };
    let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
    let host = host.trim_start_matches("www.").to_ascii_lowercase();
    let path = path.split(['?', '#']).next().unwrap_or("").trim_end_matches('/');
    if path.is_empty() {
        return unknown();
    }

    match host.as_str() {
        "pobb.in" => BuildSource::CodeUrl(format!("https://pobb.in/pob/{path}")),
        "maxroll.gg" => {
            if let Some(id) = path.strip_prefix("poe2/pob/") {
                BuildSource::CodeUrl(format!("https://maxroll.gg/poe2/api/pob/{}", first_segment(id)))
            } else if let Some(id) = path.strip_prefix("poe2/planner/") {
                BuildSource::MaxrollPlannerUrl(format!(
                    "https://planners.maxroll.gg/profiles/poe2/{}",
                    first_segment(id)
                ))
            } else {
                BuildSource::Unsupported {
                    site: "Maxroll",
                    advice: "Open the guide's planner or Path of Building link and paste that instead.",
                }
            }
        }
        "poe2db.tw" => match path.strip_prefix("pob/") {
            Some(id) => BuildSource::CodeUrl(format!("https://poe2db.tw/pob/{}/raw", first_segment(id))),
            None => unknown(),
        },
        "pastebin.com" => {
            let id = path.strip_prefix("raw/").unwrap_or(path);
            BuildSource::CodeUrl(format!("https://pastebin.com/raw/{}", first_segment(id)))
        }
        "rentry.co" => BuildSource::CodeUrl(format!("https://rentry.co/paste/{}/raw", first_segment(path))),
        "poe.ninja" | "poe2.ninja" => BuildSource::Unsupported {
            site: "poe.ninja",
            advice: "poe.ninja's build and Path of Building endpoints are internal and not \
                     available to other tools. Use its Path of Building or .build export \
                     and import that instead.",
        },
        "mobalytics.gg" => BuildSource::Unsupported {
            site: "Mobalytics",
            advice: "Mobalytics pages can't be fetched by tools. Use the guide's in-game \
                     Build Planner export (.build files) and import those.",
        },
        _ => unknown(),
    }
}

fn unknown() -> BuildSource {
    BuildSource::Unsupported {
        site: "unknown",
        advice: "Paste a Path of Building code, or a pobb.in, Maxroll, poe2db, pastebin or rentry link.",
    }
}

fn first_segment(path: &str) -> &str {
    path.split('/').next().unwrap_or(path)
}

/// `https://www.youtube.com/redirect?...&q=<url>` → `<url>` (PoB does the same).
fn unwrap_redirect(input: &str) -> Option<String> {
    if !(input.contains("youtube.com/redirect") || input.contains("google.com/url")) {
        return None;
    }
    let query = input.split_once('?')?.1;
    let value = query.split('&').find_map(|pair| pair.strip_prefix("q="))?;
    Some(percent_decode(value))
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
            if let Some(b) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    const STORMWEAVER_0_2: &str = include_str!("../tests/fixtures/stormweaver_0_2.pob");
    const DEADEYE_0_5: &str = include_str!("../tests/fixtures/deadeye_0_5.pob");

    #[test]
    fn decodes_padded_and_unpadded_real_codes() {
        for code in [STORMWEAVER_0_2, DEADEYE_0_5] {
            let xml = decode(code).unwrap();
            assert!(xml.contains("<PathOfBuilding2>"));
        }
    }

    #[test]
    fn encode_round_trips() {
        let xml = decode(DEADEYE_0_5).unwrap();
        assert_eq!(decode(&encode(&xml)).unwrap(), xml);
    }

    #[test]
    fn standard_alphabet_and_whitespace_are_tolerated() {
        let xml = "<PathOfBuilding2></PathOfBuilding2>";
        let code = encode(xml).replace('-', "+").replace('_', "/");
        let spaced = format!("  {}\n{}  ", &code[..10], &code[10..]);
        assert_eq!(decode(&spaced).unwrap(), xml);
    }

    #[test]
    fn resolves_share_links_like_pob() {
        assert_eq!(
            resolve("https://pobb.in/zR3rtheeI4fI"),
            BuildSource::CodeUrl("https://pobb.in/pob/zR3rtheeI4fI".into())
        );
        assert_eq!(
            resolve("https://pobb.in/u/Someone/abc123"),
            BuildSource::CodeUrl("https://pobb.in/pob/u/Someone/abc123".into())
        );
        assert_eq!(
            resolve("pob2://pobbin/mK7XRwbeZdUS"),
            BuildSource::CodeUrl("https://pobb.in/pob/mK7XRwbeZdUS".into())
        );
        assert_eq!(
            resolve("https://maxroll.gg/poe2/pob/ny6806ry"),
            BuildSource::CodeUrl("https://maxroll.gg/poe2/api/pob/ny6806ry".into())
        );
        assert_eq!(
            resolve("https://maxroll.gg/poe2/planner/5mmnx0qq#planner&passives"),
            BuildSource::MaxrollPlannerUrl("https://planners.maxroll.gg/profiles/poe2/5mmnx0qq".into())
        );
        assert_eq!(
            resolve("https://pastebin.com/AbC123"),
            BuildSource::CodeUrl("https://pastebin.com/raw/AbC123".into())
        );
        assert_eq!(
            resolve("https://rentry.co/xyz"),
            BuildSource::CodeUrl("https://rentry.co/paste/xyz/raw".into())
        );
        assert_eq!(
            resolve("https://poe2db.tw/pob/q1w2"),
            BuildSource::CodeUrl("https://poe2db.tw/pob/q1w2/raw".into())
        );
        assert!(matches!(
            resolve("https://poe.ninja/poe2/builds/x"),
            BuildSource::Unsupported { site: "poe.ninja", .. }
        ));
        assert!(matches!(
            resolve("https://mobalytics.gg/poe-2/builds/x"),
            BuildSource::Unsupported { site: "Mobalytics", .. }
        ));
        assert_eq!(resolve("eNrtfWtT4"), BuildSource::Code("eNrtfWtT4".into()));
    }

    #[test]
    fn unwraps_youtube_redirects() {
        assert_eq!(
            resolve("https://www.youtube.com/redirect?event=video_description&q=https%3A%2F%2Fpobb.in%2FzR3rtheeI4fI"),
            BuildSource::CodeUrl("https://pobb.in/pob/zR3rtheeI4fI".into())
        );
    }
}
