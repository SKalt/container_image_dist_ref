use container_image_dist_ref::RefStr;
fn escape(s: &str) -> String {
    s.replace("\t", "\\t")
        .replace("\n", "\\n")
        .replace("\r", "\\r")
}
fn main() {
    // read stdin to completion
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();
    let input = input.trim_end_matches(&['\r', '\n']);
    let result = RefStr::new(input);
    match result {
        Ok(ref_str) => {
            let input = escape(&input);
            let name = escape(ref_str.name());
            let domain = escape(ref_str.domain().unwrap_or(""));
            let path = escape(ref_str.path());
            let tag = escape(ref_str.tag().unwrap_or(""));
            let digest_algo = escape(ref_str.digest().map(|d| d.algorithm().src()).unwrap_or(""));
            let digest_encoded = escape(ref_str.digest().map(|d| d.encoded().src()).unwrap_or(""));
            let err = "";
            println!(
                "{input}\t{name}\t{domain}\t{path}\t{tag}\t{digest_algo}\t{digest_encoded}\t{err}"
            );
            std::process::exit(0);
        }
        Err(e) => {
            let input = escape(&input);
            let name = "";
            let domain = "";
            let path = "";
            let tag = "";
            let digest_algo = "";
            let digest_encoded = "";
            let err = format!("{:?} @ {}", e.kind(), e.index());
            println!(
                "{input}\t{name}\t{domain}\t{path}\t{tag}\t{digest_algo}\t{digest_encoded}\t{err}"
            );
            std::process::exit(1);
        }
    }
}
