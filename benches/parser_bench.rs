use criterion::{Criterion, criterion_group, criterion_main};
use xscraper::parser::{parse_tweets, parse_users};

#[path = "../tests/support/sample_payloads.rs"]
mod sample_payloads;

fn parser_bench(c: &mut Criterion) {
    let search = sample_payloads::search_payload();
    let user = sample_payloads::user_payload();

    c.bench_function("parse_search_tweets", |b| {
        b.iter(|| parse_tweets(std::hint::black_box(&search), 20))
    });
    c.bench_function("parse_user_by_login", |b| {
        b.iter(|| parse_users(std::hint::black_box(&user), -1))
    });
}

criterion_group!(benches, parser_bench);
criterion_main!(benches);
