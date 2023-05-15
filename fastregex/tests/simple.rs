use fastregex::matcher;

matcher!(
    https_matcher,
    "https?://(([A-Za-z.]+/)+([A-Za-z.]+)?)|([A-Za-z.]+)"
);

#[test]
fn test_matcher() {
    assert_eq!(https_matcher("http://test"), true);
    assert_eq!(https_matcher("http:/"), false);
    assert_eq!(https_matcher("http://"), false);
    assert_eq!(
        https_matcher("http://example.com/this/is/a/test/page.html"),
        true
    );
    assert_eq!(https_matcher(""), false);
    assert_eq!(
        https_matcher("The quick brown fox jumped over the lazy dog."),
        false
    );
}
