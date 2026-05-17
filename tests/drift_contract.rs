use xscraper::drift::build_report;

#[test]
fn drift_report_extracts_operations_and_xclid_chunk() {
    let html = r#"https://abs.twimg.com/responsive-web/client-web/main.abc123.js
_.u=e=>""+(({59924:"ondemand.s",61093:"bundle.UserAbout"})[e]||e)+"."+({59924:"f7a413c",61093:"39d4cf7"})[e]+"a.js""#;
    let main = r#"277523(e){e.exports={queryId:"Yw6L66Pw54NHKuq4Dp7b4Q",operationName:"SearchTimeline",operationType:"query",metadata:{featureSwitches:["rweb_video_screen_enabled","view_counts_everywhere_api_enabled"],fieldToggles:["withArticlePlainText","withGrokAnalyze"]}}},486917(e){e.exports={queryId:"IGgvgiOx4QZndDHuD3x9TQ",operationName:"UserByScreenName",operationType:"query",metadata:{featureSwitches:[],fieldToggles:[]}}}"#;

    let report = build_report(html, main);

    assert_eq!(
        report.xclid_script.as_deref(),
        Some("https://abs.twimg.com/responsive-web/client-web/ondemand.s.f7a413ca.js")
    );
    assert!(report.search_feature_flags.contains(&"rweb_video_screen_enabled".to_string()));
    assert!(report.search_field_toggles.contains(&"withGrokAnalyze".to_string()));
    let search =
        report.operations_checked.iter().find(|item| item.operation == "SearchTimeline").unwrap();
    assert!(search.matches);
}
