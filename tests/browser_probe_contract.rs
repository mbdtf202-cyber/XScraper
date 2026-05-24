use serde_json::json;
use xscraper::browser_probe::{
    BrowserCookie, build_browser_drift_report, parse_graphql_xhr_events,
};
use xscraper::gql::OP_SEARCH_TIMELINE;
use xscraper::operations::operation_request;

#[test]
fn browser_probe_extracts_graphql_operation_from_cdp_event() {
    let event = json!({
        "method": "Network.requestWillBeSent",
        "params": {
            "request": {
                "url": format!("https://x.com/i/api/graphql/{OP_SEARCH_TIMELINE}?variables=%7B%22rawQuery%22%3A%22rust%22%7D&features=%7B%22view_counts_everywhere_api_enabled%22%3Atrue%7D&fieldToggles=%7B%22withArticlePlainText%22%3Afalse%7D"),
                "method": "GET"
            }
        }
    });

    let observed = parse_graphql_xhr_events(&[event]).unwrap();

    assert_eq!(observed.len(), 1);
    assert_eq!(observed[0].operation, "SearchTimeline");
    assert_eq!(observed[0].op, OP_SEARCH_TIMELINE);
    assert_eq!(observed[0].variables["rawQuery"], "rust");
    assert_eq!(observed[0].features["view_counts_everywhere_api_enabled"], true);
    assert_eq!(observed[0].field_toggles["withArticlePlainText"], false);
}

#[test]
fn browser_probe_compares_observed_xhr_to_local_operation_request() {
    let local = operation_request("search", "rust", None).unwrap();
    let observed = parse_graphql_xhr_events(&[json!({
        "params": {
            "request": {
                "url": format!("https://x.com/i/api/graphql/{OP_SEARCH_TIMELINE}?variables=%7B%22rawQuery%22%3A%22rust%22%2C%22product%22%3A%22Latest%22%7D"),
                "method": "GET"
            }
        }
    })])
    .unwrap();

    let report = build_browser_drift_report(vec![local], observed);

    assert!(report.ok);
    assert_eq!(report.matches[0].operation, "SearchTimeline");
    assert!(report.matches[0].op_matches);
    assert!(report.matches[0].variable_diffs.is_empty());
}

#[test]
fn browser_probe_treats_unrelated_page_graphql_as_evidence_not_failure() {
    let local = operation_request("search", "rust", None).unwrap();
    let observed = parse_graphql_xhr_events(&[
        json!({
            "params": {
                "request": {
                    "url": "https://x.com/i/api/graphql/abc/DataSaverMode?variables=%7B%7D",
                    "method": "GET"
                }
            }
        }),
        json!({
            "params": {
                "request": {
                    "url": format!("https://x.com/i/api/graphql/{OP_SEARCH_TIMELINE}?variables=%7B%22rawQuery%22%3A%22rust%22%2C%22product%22%3A%22Latest%22%7D"),
                    "method": "GET"
                }
            }
        }),
    ])
    .unwrap();

    let report = build_browser_drift_report(vec![local], observed);

    assert!(report.ok);
    assert_eq!(report.unexpected_remote.len(), 1);
    assert_eq!(report.unexpected_remote[0].operation, "DataSaverMode");
}

#[test]
fn browser_cookie_builds_x_cookie_for_cdp_injection() {
    let cookie = BrowserCookie::x_com("ct0", "csrf");

    assert_eq!(cookie.name, "ct0");
    assert_eq!(cookie.value, "csrf");
    assert_eq!(cookie.domain, ".x.com");
    assert_eq!(cookie.path, "/");
    assert!(cookie.secure);
}
