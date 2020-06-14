fn main() {
    let contents = std::fs::read("benches/html/struct.CaptureMatches.html").unwrap();

    let extracted = cratesfyi::utils::extract_head_and_body(&contents).unwrap();
    let _head = extracted.head_node();
    let _body = extracted.body_node();
}
