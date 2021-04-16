pub fn has_support(c: char) -> bool {
    0xAC00 <= c as u32 && c as u32 <= 0xD7A3 && ((c as u32 - 0xAC00) % 28 != 0)
}
