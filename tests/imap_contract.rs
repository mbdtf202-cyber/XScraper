use xscraper::imap::{extract_email_code, imap_domain_for_email};

#[test]
fn imap_domain_mapping_resolves_common_providers() {
    assert_eq!(imap_domain_for_email("user@yahoo.com"), "imap.mail.yahoo.com");
    assert_eq!(imap_domain_for_email("user@icloud.com"), "imap.mail.me.com");
    assert_eq!(imap_domain_for_email("user@outlook.com"), "imap-mail.outlook.com");
    assert_eq!(imap_domain_for_email("user@hotmail.com"), "imap-mail.outlook.com");
    assert_eq!(imap_domain_for_email("user@example.com"), "imap.example.com");
}

#[test]
fn email_code_extraction_accepts_confirmation_subjects() {
    assert_eq!(
        extract_email_code("From info@x.com\nSubject: Your Twitter confirmation code is 123456"),
        Some("123456".into())
    );
    assert_eq!(extract_email_code("confirmation code is missing"), None);
}
