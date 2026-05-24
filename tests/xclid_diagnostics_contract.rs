use xscraper::xclid::{build_asset_diagnostics, diagnose_from_html};

#[test]
fn xclid_diagnostics_explains_successful_asset_parse() {
    let html = r#"
    <html><head><meta name="twitter-site-verification" content="AQIDBAUG"></head>
    <script>_.u=e=>""+(({59924:"ondemand.s",61093:"bundle.UserAbout"})[e]||e)+"."+({59924:"f7a413c",61093:"39d4cf7"})[e]+"a.js"</script>
    </html>
    "#;

    let report = build_asset_diagnostics(html);

    assert!(report.ok);
    assert_eq!(
        report.xclid_script.as_deref(),
        Some("https://abs.twimg.com/responsive-web/client-web/ondemand.s.f7a413ca.js")
    );
    assert!(report.chunk_ids.contains(&"59924".to_string()));
    assert!(report.failure_stage.is_none());
}

#[test]
fn xclid_diagnostics_reports_missing_chunk_marker_stage() {
    let report = diagnose_from_html("<html></html>");

    assert!(!report.ok);
    assert_eq!(report.failure_stage.as_deref(), Some("chunk-map"));
    assert!(report.failures.iter().any(|failure| failure.contains("chunk marker")));
}
