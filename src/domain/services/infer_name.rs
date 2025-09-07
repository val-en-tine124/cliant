use infer;

pub fn get_extension(buf:&[u8])->Option<String> {
        let inferred_type = infer::get(buf);
        if let Some(inferred_type) = inferred_type {
        return Some(inferred_type.extension().to_string());
    }
    None
    }