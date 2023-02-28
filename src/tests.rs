macro_rules! include_test_data {
    ($e:expr) => {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            concat!("/tests/data/", $e)
        ))
    };
}
