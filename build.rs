use std::io::Write;
use std::path::PathBuf;

fn main() {
    generate_long_file();
    generate_wide_file();
    generate_large_file();
}

/// Generates a 100_000 character long file.
fn generate_long_file() {
    let mut file = std::fs::File::create(PathBuf::from("./test-data/long.log")).unwrap();

    for i in 0..100_000 {
        let value = i.to_string();
        file.write(value.as_bytes()).unwrap();

        if i != 99_999 {
            file.write(&[0xA]).unwrap();
        }
    }
}

/// Generates a 100_000 character wide file.
fn generate_wide_file() {
    let mut file = std::fs::File::create(PathBuf::from("./test-data/wide.log")).unwrap();
    file.write("a".repeat(100_000).as_bytes()).unwrap();
}

/// Generates a ~1921 MB file.
fn generate_large_file() {
    let mut file = std::fs::File::create(PathBuf::from("./test-data/large.log")).unwrap();

    for i in 0..100_000 {
        file.write("a".repeat(20_000).as_bytes()).unwrap();

        if i != 99_999 {
            file.write(&[0xA]).unwrap();
        }
    }
}
