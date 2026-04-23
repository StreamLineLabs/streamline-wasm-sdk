//! Input validation helpers.

/// Validate a Kafka topic name according to the protocol rules:
/// - Must not be empty
/// - Max 249 characters
/// - Only ASCII alphanumeric, '.', '_', '-' allowed
/// - Must not be "." or ".."
pub fn validate_topic_name(topic: &str) -> Result<(), String> {
    if topic.is_empty() {
        return Err("Topic name must not be empty".to_string());
    }
    if topic.len() > 249 {
        return Err("Topic name exceeds max length of 249".to_string());
    }
    if topic == "." || topic == ".." {
        return Err("Topic name must not be '.' or '..'".to_string());
    }
    if !topic
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
    {
        return Err(format!(
            "Topic name contains invalid characters: {}",
            topic
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_simple_name() {
        assert!(validate_topic_name("orders").is_ok());
    }

    #[test]
    fn valid_with_dots() {
        assert!(validate_topic_name("my.topic.name").is_ok());
    }

    #[test]
    fn valid_with_underscores() {
        assert!(validate_topic_name("my_topic_name").is_ok());
    }

    #[test]
    fn valid_with_hyphens() {
        assert!(validate_topic_name("my-topic-name").is_ok());
    }

    #[test]
    fn valid_mixed_chars() {
        assert!(validate_topic_name("org.example.topic-v2_final").is_ok());
    }

    #[test]
    fn valid_single_char() {
        assert!(validate_topic_name("a").is_ok());
    }

    #[test]
    fn valid_max_length() {
        let name = "a".repeat(249);
        assert!(validate_topic_name(&name).is_ok());
    }

    #[test]
    fn reject_empty() {
        let err = validate_topic_name("").unwrap_err();
        assert!(err.contains("empty"), "got: {err}");
    }

    #[test]
    fn reject_exceeds_max_length() {
        let name = "a".repeat(250);
        let err = validate_topic_name(&name).unwrap_err();
        assert!(err.contains("249"), "got: {err}");
    }

    #[test]
    fn reject_dot() {
        let err = validate_topic_name(".").unwrap_err();
        assert!(err.contains("'.'"), "got: {err}");
    }

    #[test]
    fn reject_dotdot() {
        let err = validate_topic_name("..").unwrap_err();
        assert!(err.contains("'..'"), "got: {err}");
    }

    #[test]
    fn reject_space() {
        let err = validate_topic_name("my topic").unwrap_err();
        assert!(err.contains("invalid characters"), "got: {err}");
    }

    #[test]
    fn reject_slash() {
        let err = validate_topic_name("my/topic").unwrap_err();
        assert!(err.contains("invalid characters"), "got: {err}");
    }

    #[test]
    fn reject_colon() {
        let err = validate_topic_name("my:topic").unwrap_err();
        assert!(err.contains("invalid characters"), "got: {err}");
    }

    #[test]
    fn reject_unicode() {
        let err = validate_topic_name("topïc").unwrap_err();
        assert!(err.contains("invalid characters"), "got: {err}");
    }

    #[test]
    fn reject_at_sign() {
        let err = validate_topic_name("user@topic").unwrap_err();
        assert!(err.contains("invalid characters"), "got: {err}");
    }

    #[test]
    fn three_dots_is_valid() {
        assert!(validate_topic_name("...").is_ok());
    }

    #[test]
    fn dot_prefixed_is_valid() {
        assert!(validate_topic_name(".hidden").is_ok());
    }
}
