use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use fastregex::matcher;
use regex::Regex;

matcher!(
    https_matcher,
    "https?://(([A-Za-z.]+/)+([A-Za-z.]+)?)|([A-Za-z.]+)"
);

fn bench_comparatively(c: &mut Criterion) {
    let regex = Regex::new("^[A-Z]https?://(([A-Za-z.]+/)+([A-Za-z.]+)?)|([A-Za-z.]+)$").unwrap();

    let haystack = [
        "http://test",
        "http:/",
        "http://",
        "http://example.com/this/is/a/test/page.html",
        "The quick brown fox jumped over the lazy dog.",
    ];
    //let haystack = ["The quick brown fox jumped over the lazy dog."];

    for haystack in haystack {
        let mut group = c.benchmark_group(haystack);
        group.bench_with_input(BenchmarkId::new("Fastregex", haystack), haystack, |b, i| {
            b.iter(|| https_matcher(black_box(i)))
        });
        group.bench_with_input(
            BenchmarkId::new("Traditional Regex", haystack),
            haystack,
            |b, i| b.iter(|| regex.is_match(black_box(i))),
        );
        group.finish();
    }
}

criterion_group!(benches, bench_comparatively);
criterion_main!(benches);
