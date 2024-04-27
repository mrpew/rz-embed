lazy_static! {
    static ref const_name: Vec<u8> = {
        let compressed_data: &[u8] = include_bytes!("compressed_source");
        let mut decoder = GzDecoder::new(compressed_data);
        let mut decompressed_data = Vec::new();
        decoder.read_to_end(&mut decompressed_data).unwrap();
        decompressed_data
    };
}
